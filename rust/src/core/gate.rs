//! Byte-signature scanning and patching engine.
//! Supports regex searching over binary slices, anchored literal pre-filtering (SIMD),
//! executable section bounds, and early-exit on the first match.

use regex::bytes::{Regex, RegexBuilder};
use crate::core::arch::Arch;
use crate::core::bounds::CodeBounds;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateStatus {
    Patched,
    Unpatched,
    Unknown,
}

impl GateStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            GateStatus::Patched => "patched",
            GateStatus::Unpatched => "unpatched",
            GateStatus::Unknown => "unknown",
        }
    }
}

pub struct Gate {
    pub sig: Regex,
    pub patched: Regex,
    pub fix: &'static [u8],
    pub offset: usize,
    pub desc: &'static str,
    pub arch: Option<Arch>,
    pub anchor_sig: Option<(&'static [u8], usize)>,
    pub anchor_patched: Option<(&'static [u8], usize)>,
    pub single: bool,
}

impl Gate {
    pub fn new(
        sig_pat: &str,
        patched_pat: &str,
        fix: &'static [u8],
        offset: usize,
        desc: &'static str,
        arch: Option<Arch>,
        anchor_sig: Option<(&'static [u8], usize)>,
        anchor_patched: Option<(&'static [u8], usize)>,
        single: bool,
    ) -> Result<Self, regex::Error> {
        let sig = RegexBuilder::new(sig_pat)
            .unicode(false)
            .dot_matches_new_line(true)
            .build()?;
        let patched = RegexBuilder::new(patched_pat)
            .unicode(false)
            .dot_matches_new_line(true)
            .build()?;
        Ok(Self {
            sig,
            patched,
            fix,
            offset,
            desc,
            arch,
            anchor_sig,
            anchor_patched,
            single,
        })
    }

    fn search_anchored(
        pat: &Regex,
        data: &[u8],
        anchor: Option<(&'static [u8], usize)>,
        offset: usize,
        limit: Option<usize>,
        bounds: Option<CodeBounds>,
    ) -> Vec<usize> {
        let (start_search, end_search) = match bounds {
            Some(b) => (b.start.min(data.len()), b.end.min(data.len())),
            None => (0, data.len()),
        };
        if start_search >= end_search {
            return Vec::new();
        }

        let slice = &data[start_search..end_search];

        let Some((literal, anchor_off)) = anchor else {
            let mut out = Vec::new();
            for m in pat.find_iter(slice) {
                out.push(start_search + m.start() + offset);
                if let Some(lim) = limit {
                    if out.len() >= lim {
                        return out;
                    }
                }
            }
            return out;
        };

        let finder = memchr::memmem::Finder::new(literal);
        let mut out = Vec::new();
        let mut curr = 0;
        let mut last_end = 0;

        while curr + literal.len() <= slice.len() {
            let rel_pos = match finder.find(&slice[curr..]) {
                Some(p) => p,
                None => break,
            };
            let pos = curr + rel_pos;
            if pos >= anchor_off {
                let match_start = pos - anchor_off;
                if match_start >= last_end {
                    let window_len = 128.min(slice.len() - match_start);
                    let cand_window = &slice[match_start..match_start + window_len];
                    if let Some(m) = pat.find(cand_window) {
                        if m.start() == 0 {
                            out.push(start_search + match_start + offset);
                            if let Some(lim) = limit {
                                if out.len() >= lim {
                                    return out;
                                }
                            }
                            last_end = match_start + m.end();
                        }
                    }
                }
            }
            curr = pos + 1;
        }

        out
    }

    pub fn find(
        &self,
        data: &[u8],
        target_arch: Option<Arch>,
        limit_patched: Option<usize>,
        bounds: Option<CodeBounds>,
    ) -> Result<(GateStatus, Vec<usize>), String> {
        if let (Some(ga), Some(ta)) = (self.arch, target_arch) {
            if ga != ta {
                return Err(format!("gate '{}' is {:?}, binary is {:?}", self.desc, ga, ta));
            }
        }

        if self.single {
            return self.find_single(data, bounds);
        }

        // 1. Search patched
        let patched_offsets = Self::search_anchored(
            &self.patched,
            data,
            self.anchor_patched,
            self.offset,
            limit_patched,
            bounds,
        );
        if !patched_offsets.is_empty() {
            return Ok((GateStatus::Patched, patched_offsets));
        }

        // 2. Search unpatched
        let unpatched_offsets = Self::search_anchored(
            &self.sig,
            data,
            self.anchor_sig,
            self.offset,
            None,
            bounds,
        );
        if !unpatched_offsets.is_empty() {
            return Ok((GateStatus::Unpatched, unpatched_offsets));
        }

        // 3. Fallback to full file if bounds were active
        if bounds.is_some() {
            return self.find(data, target_arch, limit_patched, None);
        }

        // 4. Fallback to unanchored regex find_iter over entire file
        let patched_fallback: Vec<usize> = self.patched
            .find_iter(data)
            .map(|m| m.start() + self.offset)
            .collect();
        if !patched_fallback.is_empty() {
            return Ok((GateStatus::Patched, patched_fallback));
        }

        let unpatched_fallback: Vec<usize> = self.sig
            .find_iter(data)
            .map(|m| m.start() + self.offset)
            .collect();
        if !unpatched_fallback.is_empty() {
            return Ok((GateStatus::Unpatched, unpatched_fallback));
        }

        Err(format!("gate '{}' signature not found", self.desc))
    }

    fn find_single(
        &self,
        data: &[u8],
        bounds: Option<CodeBounds>,
    ) -> Result<(GateStatus, Vec<usize>), String> {
        // Patched check
        let patched_cands = Self::search_anchored(
            &self.patched,
            data,
            self.anchor_patched,
            self.offset,
            Some(1),
            bounds,
        );
        if !patched_cands.is_empty() {
            return Ok((GateStatus::Patched, patched_cands));
        }

        // Unpatched check
        let unpatched_cands = Self::search_anchored(
            &self.sig,
            data,
            self.anchor_sig,
            self.offset,
            Some(2),
            bounds,
        );
        if unpatched_cands.len() > 1 {
            return Err(format!("gate '{}' signature is not unique", self.desc));
        }
        if !unpatched_cands.is_empty() {
            return Ok((GateStatus::Unpatched, unpatched_cands));
        }

        if bounds.is_some() {
            return self.find_single(data, None);
        }

        // Final fallback: standard regex search
        if let Some(m) = self.patched.find(data) {
            return Ok((GateStatus::Patched, vec![m.start() + self.offset]));
        }
        if let Some(m) = self.sig.find(data) {
            if self.sig.find_at(data, m.end()).is_some() {
                return Err(format!("gate '{}' signature is not unique", self.desc));
            }
            return Ok((GateStatus::Unpatched, vec![m.start() + self.offset]));
        }

        Err(format!("gate '{}' signature not found", self.desc))
    }
}

pub struct MultiGate {
    pub gates: Vec<Gate>,
    pub desc: &'static str,
}

impl MultiGate {
    pub fn new(gates: Vec<Gate>, desc: &'static str) -> Self {
        Self { gates, desc }
    }

    pub fn resolve(
        &self,
        data: &[u8],
        target_arch: Option<Arch>,
        bounds: Option<CodeBounds>,
    ) -> Result<(GateStatus, usize, &'static str, &'static [u8]), String> {
        let mut last_err = String::new();
        let mut skipped = Vec::new();

        for gate in &self.gates {
            if let (Some(ga), Some(ta)) = (gate.arch, target_arch) {
                if ga != ta {
                    skipped.push(gate);
                    continue;
                }
            }
            match gate.find(data, target_arch, Some(1), bounds) {
                Ok((status, offsets)) if !offsets.is_empty() => {
                    return Ok((status, offsets[0], gate.desc, gate.fix));
                }
                Ok(_) => {}
                Err(e) => last_err = e,
            }
        }

        for gate in skipped {
            match gate.find(data, None, Some(1), bounds) {
                Ok((status, offsets)) if !offsets.is_empty() => {
                    return Ok((status, offsets[0], gate.desc, gate.fix));
                }
                Ok(_) => {}
                Err(e) => last_err = e,
            }
        }

        Err(if last_err.is_empty() {
            format!("no gate in '{}' matched", self.desc)
        } else {
            last_err
        })
    }
}


