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
    // SManga fields

    Candidate {
        name: "url",
        signals: &[
            Signal { kind_tag: SignalTag::PassedToKnown("setUrl", 0), weight: 1.0 },
            Signal { kind_tag: SignalTag::GetterShaped, weight: 0.1 },
        ],
    },
    Candidate {
        name: "title",
        signals: &[
            Signal { kind_tag: SignalTag::PassedToKnown("setTitle", 0), weight: 1.0 },
            Signal { kind_tag: SignalTag::GetterShaped, weight: 0.1 },
        ],
    },
    Candidate {
        name: "artist",
        signals: &[
            Signal { kind_tag: SignalTag::PassedToKnown("setArtist", 0), weight: 1.0 },
            Signal { kind_tag: SignalTag::PassedToKnown("joinToString", 0), weight: 0.5 },
            Signal { kind_tag: SignalTag::GetterShaped, weight: 0.1 },
        ],
    },
    Candidate {
        name: "author",
        signals: &[
            Signal { kind_tag: SignalTag::PassedToKnown("setAuthor", 0), weight: 1.0 },
            Signal { kind_tag: SignalTag::PassedToKnown("joinToString", 0), weight: 0.5 },
            Signal { kind_tag: SignalTag::GetterShaped, weight: 0.1 },
        ],
    },
    Candidate {
        name: "description",
        signals: &[
            Signal { kind_tag: SignalTag::PassedToKnown("setDescription", 0), weight: 1.0 },
            Signal { kind_tag: SignalTag::PassedToKnown("parseBodyFragment", 0), weight: 0.9 },
            Signal { kind_tag: SignalTag::GetterShaped, weight: 0.1 },
        ],
    },
    Candidate {
        name: "genre",
        signals: &[
            Signal { kind_tag: SignalTag::PassedToKnown("setGenre", 0), weight: 1.0 },
            Signal { kind_tag: SignalTag::GetterShaped, weight: 0.1 },
        ],
    },
    Candidate {
        name: "status",
        signals: &[
            Signal { kind_tag: SignalTag::PassedToKnown("setStatus", 0), weight: 1.0 },
            Signal { kind_tag: SignalTag::ComparedToIntLiteral, weight: 0.7 },
            Signal { kind_tag: SignalTag::GetterShaped, weight: 0.1 },
        ],
    },
    Candidate {
        name: "thumbnail_url",
        signals: &[
            Signal { kind_tag: SignalTag::PassedToKnown("setThumbnail_url", 0), weight: 1.0 },
            Signal { kind_tag: SignalTag::GetterShaped, weight: 0.1 },
        ],
    },

    // SChapter fields

    Candidate {
        name: "chapter_url",
        signals: &[
            Signal { kind_tag: SignalTag::PassedToKnown("setUrl", 0), weight: 0.8 },
            Signal { kind_tag: SignalTag::AppearsAfterStringLiteral("/comic/"), weight: 0.9 },
            Signal { kind_tag: SignalTag::GetterShaped, weight: 0.1 },
        ],
    },
    Candidate {
        name: "name",
        signals: &[
            Signal { kind_tag: SignalTag::PassedToKnown("setName", 0), weight: 1.0 },
            Signal { kind_tag: SignalTag::GetterShaped, weight: 0.1 },
        ],
    },
    Candidate {
        name: "date_upload",
        signals: &[
            Signal { kind_tag: SignalTag::PassedToKnown("setDate_upload", 0), weight: 1.0 },
            Signal { kind_tag: SignalTag::PassedToKnown("tryParse", 1), weight: 0.9 },
            Signal { kind_tag: SignalTag::GetterShaped, weight: 0.1 },
        ],
    },
    Candidate {
        name: "chapter_number",
        signals: &[
            Signal { kind_tag: SignalTag::PassedToKnown("setChapter_number", 0), weight: 1.0 },
            Signal { kind_tag: SignalTag::GetterShaped, weight: 0.1 },
        ],
    },
    Candidate {
        name: "scanlator",
        signals: &[
            Signal { kind_tag: SignalTag::PassedToKnown("setScanlator", 0), weight: 1.0 },
            // it.groups.joinToString() → result passed to setScanlator
            Signal { kind_tag: SignalTag::PassedToKnown("joinToString", 0), weight: 0.4 },
            Signal { kind_tag: SignalTag::GetterShaped, weight: 0.1 },
        ],
    },

    // MangasPage fields

    Candidate {
        name: "mangas",
        signals: &[
            Signal { kind_tag: SignalTag::StoredFrom("ArrayList"), weight: 0.8 },
            Signal { kind_tag: SignalTag::ResultPushedToList, weight: 0.6 },
            Signal { kind_tag: SignalTag::Iterated, weight: 0.4 },
        ],
    },
    Candidate {
        name: "hasNextPage",
        signals: &[
            Signal { kind_tag: SignalTag::UsedAsLoopCondition, weight: 1.0 },
            Signal { kind_tag: SignalTag::ComparedToIntLiteral, weight: 0.4 },
            Signal { kind_tag: SignalTag::GetterShaped, weight: 0.1 },
        ],
    },

    // Page fields
    Candidate {
        name: "imageUrl",
        signals: &[
            Signal { kind_tag: SignalTag::PassedToKnown("setImageUrl", 0), weight: 1.0 },
            // or as ctor arg to Page
            Signal { kind_tag: SignalTag::StoredFrom("Page"), weight: 0.6 },
            Signal { kind_tag: SignalTag::GetterShaped, weight: 0.1 },
        ],
    },

    // Response wrapper fields (Data<T>, SearchResponse, ChapterList)
    Candidate {
        name: "data",
        signals: &[
            Signal { kind_tag: SignalTag::PassedToKnown("collectionSizeOrDefault", 0), weight: 1.0 },
            Signal { kind_tag: SignalTag::PassedToKnown("toMutableList", 0),           weight: 1.0 },
            Signal { kind_tag: SignalTag::PassedToKnown("addAll", 0),                  weight: 0.4 },
            Signal { kind_tag: SignalTag::ReceiverOf("size"),                           weight: 0.8 },
            Signal { kind_tag: SignalTag::CalledOnIteratorNext,                         weight: 0.5 },
            Signal { kind_tag: SignalTag::Iterated,                                    weight: 0.3 },
            Signal { kind_tag: SignalTag::ReceiverOf("iterator"),                      weight: 0.2 },
            Signal { kind_tag: SignalTag::GetterShaped,                                weight: 0.05 },
        ],
    },
    Candidate {
        name: "cursor",
        signals: &[
            Signal { kind_tag: SignalTag::NullChecked, weight: 0.5 },
            Signal { kind_tag: SignalTag::AppearsAfterStringLiteral("cursor"), weight: 0.9 },
            Signal { kind_tag: SignalTag::NullChecked, weight: 0.5 },
            Signal { kind_tag: SignalTag::GetterShaped, weight: 0.1 },
        ],
    },

    // BrowseComic / Chapter element accessors
    Candidate {
        name: "toSManga",
        signals: &[
            Signal { kind_tag: SignalTag::ResultPushedToList,              weight: 0.5 },
            Signal { kind_tag: SignalTag::CalledOnIteratorNext,            weight: 0.5 },
            Signal { kind_tag: SignalTag::PassedToKnown("push", 0),       weight: 0.4 },
            Signal { kind_tag: SignalTag::ResultPushedToList, weight: 0.9 },
        ],
    },
    Candidate {
        name: "hid",
        signals: &[
            Signal { kind_tag: SignalTag::AppearsBeforeStringLiteral("-chapter-"), weight: 0.9 },
            Signal { kind_tag: SignalTag::CalledOnIteratorNext,                    weight: 0.1 },
            Signal { kind_tag: SignalTag::GetterShaped,                            weight: 0.05 },
        ],
    },
    Candidate {
        name: "chap",
        signals: &[
            Signal { kind_tag: SignalTag::AppearsAfterStringLiteral("-chapter-"), weight: 0.9 },
            Signal { kind_tag: SignalTag::AppearsAfterStringLiteral("Ch. "),      weight: 0.9 },
            Signal { kind_tag: SignalTag::CalledOnIteratorNext,                   weight: 0.1 },
            Signal { kind_tag: SignalTag::PassedToKnown("append", 0),             weight: 0.1 },
            Signal { kind_tag: SignalTag::GetterShaped,                           weight: 0.05 },
        ],
    },
    Candidate {
        name: "lang",
        signals: &[
            Signal { kind_tag: SignalTag::ComparedToStringLiteral("en"),          weight: 1.0 },
            Signal { kind_tag: SignalTag::ComparedToStringLiteral("ja"),          weight: 1.0 },
            Signal { kind_tag: SignalTag::ComparedToStringLiteral("ko"),          weight: 1.0 },
            Signal { kind_tag: SignalTag::ComparedToStringLiteral("zh"),          weight: 1.0 },
            Signal { kind_tag: SignalTag::AppearsAfterStringLiteral("-"),         weight: 0.6 },
            Signal { kind_tag: SignalTag::PassedToKnown("append", 0),             weight: 0.3 },
            Signal { kind_tag: SignalTag::CalledOnIteratorNext,                   weight: 0.1 },
            Signal { kind_tag: SignalTag::GetterShaped,                           weight: 0.05 },
        ],
    },
    Candidate {
        name: "vol",
        signals: &[
            Signal { kind_tag: SignalTag::AppearsAfterStringLiteral("Vol. "),  weight: 0.9 },
            Signal { kind_tag: SignalTag::NullChecked,                         weight: 0.1 },
            Signal { kind_tag: SignalTag::CalledOnIteratorNext,                weight: 0.1 },
            Signal { kind_tag: SignalTag::GetterShaped,                        weight: 0.05 },
        ],
    },
    Candidate {
        name: "chapter_title",
        signals: &[
            Signal { kind_tag: SignalTag::AppearsAfterStringLiteral(": "),  weight: 0.9 },
            Signal { kind_tag: SignalTag::NullChecked,                      weight: 0.1 },
            Signal { kind_tag: SignalTag::CalledOnIteratorNext,             weight: 0.1 },
            Signal { kind_tag: SignalTag::GetterShaped,                     weight: 0.05 },
        ],
    },
    Candidate {
        name: "createdAt",
        signals: &[
            Signal { kind_tag: SignalTag::PassedToKnown("tryParse", 1),  weight: 1.0 },
            Signal { kind_tag: SignalTag::PassedToKnown("a", 1),         weight: 0.05 },
            Signal { kind_tag: SignalTag::CalledOnIteratorNext,           weight: 0.1 },
            Signal { kind_tag: SignalTag::GetterShaped,                   weight: 0.05 },
        ],
    },
    Candidate {
        name: "groups",
        signals: &[
            Signal { kind_tag: SignalTag::PassedToKnown("joinToString", 0),         weight: 0.8 },
            Signal { kind_tag: SignalTag::PassedToKnown("joinToString$default", 0), weight: 0.8 },
            Signal { kind_tag: SignalTag::CalledOnIteratorNext,                      weight: 0.5 },
            Signal { kind_tag: SignalTag::FlowsIntoSetter("this", "setScanlator"),  weight: 1.0 },
            Signal { kind_tag: SignalTag::GetterShaped,                              weight: 0.05 },
        ],
    },

    // ComicData fields

    Candidate {
        name: "slug",
        signals: &[
            Signal { kind_tag: SignalTag::PassedToKnown("setUrl", 0),                    weight: 0.8 },
            Signal { kind_tag: SignalTag::AppearsAfterStringLiteral("/api/comics/"),      weight: 0.9 },
            Signal { kind_tag: SignalTag::AppearsBeforeStringLiteral("/api/chapters/"),  weight: 0.7 },
            Signal { kind_tag: SignalTag::GetterShaped,                                   weight: 0.1 },
        ],
    },
    Candidate {
        name: "thumbnail",
        signals: &[
            Signal { kind_tag: SignalTag::PassedToKnown("setThumbnail_url", 0), weight: 1.0 },
            Signal { kind_tag: SignalTag::GetterShaped, weight: 0.1 },
        ],
    },
    Candidate {
        name: "translationCompleted",
        signals: &[
            Signal { kind_tag: SignalTag::NullChecked, weight: 1.0 },
            Signal { kind_tag: SignalTag::GetterShaped, weight: 0.5 },
            Signal { kind_tag: SignalTag::ComparedToIntLiteral, weight: 0.5 },
        ],
    },
    Candidate {
        name: "authors",
        signals: &[
            Signal { kind_tag: SignalTag::PassedToKnown("joinToString", 0),         weight: 0.9 },
            Signal { kind_tag: SignalTag::PassedToKnown("joinToString$default", 0), weight: 0.9 },
            Signal { kind_tag: SignalTag::PassedToKnown("setAuthor", 0),            weight: 1.0 },
            Signal { kind_tag: SignalTag::CalledOnIteratorNext,                      weight: 0.5 },
            Signal { kind_tag: SignalTag::GetterShaped,                              weight: 0.05 },
            Signal { kind_tag: SignalTag::FlowsIntoSetter("this", "setAuthor"), weight: 1.0 },
        ],
    },
    Candidate {
        name: "artists",
        signals: &[
            Signal { kind_tag: SignalTag::PassedToKnown("joinToString", 0),         weight: 0.9 },
            Signal { kind_tag: SignalTag::PassedToKnown("joinToString$default", 0), weight: 0.9 },
            Signal { kind_tag: SignalTag::PassedToKnown("setArtist", 0),            weight: 1.0 },
            Signal { kind_tag: SignalTag::CalledOnIteratorNext,                      weight: 0.5 },
            Signal { kind_tag: SignalTag::GetterShaped,                              weight: 0.05 },
        ],
    },
    Candidate {
        name: "desc",
        signals: &[
            Signal { kind_tag: SignalTag::PassedToKnown("parseBodyFragment", 0), weight: 1.0 },
            Signal { kind_tag: SignalTag::GetterShaped, weight: 0.1 },
        ],
    },
    Candidate {
        name: "titles",
        signals: &[
            Signal { kind_tag: SignalTag::ReceiverOf("isEmpty"), weight: 1.0 },  // up from 0.7
            Signal { kind_tag: SignalTag::Iterated, weight: 0.4 },
            Signal { kind_tag: SignalTag::GetterShaped, weight: 0.05 },
        ],
    },
    Candidate {
        name: "country",
        signals: &[
            Signal { kind_tag: SignalTag::ReceiverOf("hashCode"),              weight: 1.0 },
            Signal { kind_tag: SignalTag::ComparedToStringLiteral("jp"),       weight: 0.9 },
            Signal { kind_tag: SignalTag::ComparedToStringLiteral("cn"),       weight: 0.9 },
            Signal { kind_tag: SignalTag::ComparedToStringLiteral("ko"),       weight: 0.9 },
            Signal { kind_tag: SignalTag::GetterShaped,                        weight: 0.05 },
        ],
    },
    Candidate {
        name: "contentRating",
        signals: &[
            Signal { kind_tag: SignalTag::ComparedToStringLiteral("suggestive"), weight: 1.0 },
            Signal { kind_tag: SignalTag::ComparedToStringLiteral("erotica"), weight: 1.0 },
            Signal { kind_tag: SignalTag::GetterShaped, weight: 0.1 },
        ],
    },
    Candidate {
        name: "genres",
        signals: &[
            Signal { kind_tag: SignalTag::PassedToKnown("addAll", 0),                  weight: 0.9 },
            Signal { kind_tag: SignalTag::Iterated,                                    weight: 0.2 },
            Signal { kind_tag: SignalTag::PassedToKnown("collectionSizeOrDefault", 0), weight: 0.3 },
            Signal { kind_tag: SignalTag::GetterShaped,                                weight: 0.05 },
        ],
    },

    Candidate {
        name: "dateFormat",
        signals: &[
            Signal { kind_tag: SignalTag::PassedToKnown("tryParse", 0), weight: 1.0 },
            Signal { kind_tag: SignalTag::GetterShaped, weight: 0.05 },
        ],
    },
    Candidate {
        name: "preferences",
        signals: &[
            Signal { kind_tag: SignalTag::ReceiverOf("getString"), weight: 0.9 },
            Signal { kind_tag: SignalTag::ReceiverOf("getBoolean"), weight: 0.9 },
            Signal { kind_tag: SignalTag::GetterShaped, weight: 0.05 },
        ],
    },
    Candidate {
        name: "siteLang",
        signals: &[
            Signal { kind_tag: SignalTag::AppearsAfterStringLiteral("lang="), weight: 1.0 },
            Signal { kind_tag: SignalTag::GetterShaped, weight: 0.05 },
        ],
    },

    Candidate {
        name: "baseUrl",
        signals: &[
            Signal { kind_tag: SignalTag::AppearsBeforeStringLiteral("/api/comics/"), weight: 1.0 },
            Signal { kind_tag: SignalTag::AppearsBeforeStringLiteral("/api/search"), weight: 1.0 },
            Signal { kind_tag: SignalTag::AppearsBeforeStringLiteral("/comic/"), weight: 0.9 },
            Signal { kind_tag: SignalTag::GetterShaped, weight: 0.05 },
        ],
    },
    Candidate {
        name: "client",
        signals: &[
            Signal { kind_tag: SignalTag::ReceiverOf("newBuilder"), weight: 0.9 },
            Signal { kind_tag: SignalTag::GetterShaped, weight: 0.05 },
        ],
    },

    Candidate {
        name: "selected",
        signals: &[
            Signal { kind_tag: SignalTag::NullChecked, weight: 1.0 },
            Signal { kind_tag: SignalTag::PassedToKnown("addQueryParameter", 1), weight: 1.0 },
            Signal { kind_tag: SignalTag::GetterShaped, weight: 0.1 },
        ],
    },
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