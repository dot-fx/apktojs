pub struct Candidate {
    pub name:    &'static str,
    pub signals: &'static [Signal],
}

pub struct Signal {
    pub kind_tag: SignalTag,
    pub weight:   f32,
}

pub enum SignalTag {
    FlowsIntoSetter(&'static str, &'static str),
    ReceiverOf(&'static str),
    PassedToKnown(&'static str, u8),
    StoredFrom(&'static str),
    GetterShaped,
    Iterated,
    NullChecked,
    UsedAsLoopCondition,
    ResultPushedToList,
    CalledOnIteratorNext,
    AppearsBeforeStringLiteral(&'static str),
    AppearsAfterStringLiteral(&'static str),
    ComparedToIntLiteral,
    ComparedToStringLiteral(&'static str),
}

pub static CANDIDATES: &[Candidate] = &[

];

pub fn score_candidate(candidate: &Candidate, ev: &super::EvidenceSet) -> f32 {
    let mut score = 0.0_f32;
    for signal in candidate.signals {
        let best_match = ev.entries.iter()
            .filter(|obs| signal_matches(&signal.kind_tag, &obs.kind))
            .map(|obs| signal.weight * obs.weight)
            .fold(0.0_f32, f32::max);
        score += best_match.min(0.9);
    }
    score
}

pub fn signal_matches(tag: &SignalTag, kind: &super::EvidenceKind) -> bool {
    match (tag, kind) {
        (SignalTag::FlowsIntoSetter(c, m),
            super::EvidenceKind::FlowsIntoSetter { setter_class, setter_method })
        => c == setter_class && m == setter_method,

        (SignalTag::ReceiverOf(m),
            super::EvidenceKind::ReceiverOf { method })
        => m == method,

        (SignalTag::PassedToKnown(m, p),
            super::EvidenceKind::PassedToKnown { method_js_name, param_index })
        => m == method_js_name && p == param_index,

        (SignalTag::StoredFrom(t),
            super::EvidenceKind::StoredFrom { expr_type })
        => t == expr_type,

        (SignalTag::ComparedToStringLiteral(lit),
            super::EvidenceKind::ComparedToStringLiteral { literal })
        => *lit == literal,

        (SignalTag::AppearsBeforeStringLiteral(lit),
            super::EvidenceKind::AppearsBeforeStringLiteral(observed))
        => *lit == observed,

        (SignalTag::AppearsAfterStringLiteral(lit),
            super::EvidenceKind::AppearsAfterStringLiteral(observed))
        => *lit == observed,

        (SignalTag::GetterShaped,        super::EvidenceKind::GetterShaped)        => true,
        (SignalTag::Iterated,            super::EvidenceKind::Iterated)            => true,
        (SignalTag::NullChecked,         super::EvidenceKind::NullChecked)         => true,
        (SignalTag::UsedAsLoopCondition, super::EvidenceKind::UsedAsLoopCondition) => true,
        (SignalTag::ResultPushedToList,  super::EvidenceKind::ResultPushedToList)  => true,
        (SignalTag::CalledOnIteratorNext,super::EvidenceKind::CalledOnIteratorNext)=> true,
        (SignalTag::ComparedToIntLiteral,super::EvidenceKind::ComparedToIntLiteral)=> true,
        _ => false,
    }
}