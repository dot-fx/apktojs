pub fn kotlin_class_to_js(class: &str) -> String {
    match class {
        "java.lang.String"          => "String".into(),
        "java.lang.StringBuilder" => "StringBuilder".into(),
        "java.lang.Integer"         => "Number".into(),
        "java.lang.Boolean"         => "Boolean".into(),
        "kotlin.collections.ArrayList"
        | "java.util.ArrayList" => "MutableList".into(),
        "okhttp3.HttpUrl"           => "HttpUrl".into(),
        "okhttp3.Request"         => "Request".into(),
        "okhttp3.FormBody"        => "FormBody".into(),
        "eu.kanade.tachiyomi.source.model.SManga" => "SManga".into(),
        "eu.kanade.tachiyomi.source.model.SChapter" => "SChapter".into(),
        "eu.kanade.tachiyomi.source.model.Page"   => "Page".into(),
        "eu.kanade.tachiyomi.source.model.MangasPage" => "MangasPage".into(),
        c => {
            c.split('.').last().unwrap_or(c).to_string()
        }
    }
}

pub fn from_dex_type(desc: &str) -> String {
    desc.trim_start_matches('L')
        .trim_end_matches(';')
        .replace('/', ".")
}