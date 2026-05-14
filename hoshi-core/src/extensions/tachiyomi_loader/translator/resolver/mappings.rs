use regex::Regex;

pub fn well_known_method(class: &str, method: &str) -> Option<String> {
    let simple = class.split('.').last().unwrap_or(class);


    let mapped = match (simple, method) {
        // StringBuilder / buildString
        ("StringBuilder", "append")   => "append",
        ("StringBuilder", "toString") => "toString",

        ("Object", "getClass") => "getClass",


        // OkHttp Request.Builder
        ("Builder", "url")     => "url",
        ("Builder", "build")   => "build",
        ("Builder", "addHeader") => "addHeader",
        ("Builder", "header")  => "header",
        ("Builder", "post")    => "post",
        ("Builder", "get")     => "get",


        // HttpUrl.Builder
        ("Builder", "addQueryParameter") => "addQueryParameter",
        ("Builder", "addPathSegment")    => "addPathSegment",
        ("Builder", "setQueryParameter") => "setQueryParameter",
        ("Builder", "removeQueryParameter") => "removeQueryParameter",
        ("Builder", "fragment")          => "fragment",
        ("Builder", "newBuilder")        => "newBuilder",


        // String methods
        ("String", "format")   => "/* String.format → */ _fmt",
        ("String", "trim")     => "trim",
        ("String", "split")    => "split",
        ("String", "contains") => "includes",
        ("String", "isEmpty")  => "isBlank",
        ("String", "replace")  => "replace",
        ("String", "lowercase")=> "toLowerCase",
        ("String", "uppercase")=> "toUpperCase",


        // JSONObject / JSONArray
        ("JSONObject", "getString") => "/* json */ getString",
        ("JSONObject", "getInt")    => "/* json */ getInt",
        ("JSONObject", "optString") => "/* json */ optString",
        ("JSONArray",  "length")    => "length",
        ("JSONArray",  "getJSONObject") => "getJSONObject",


        // Jsoup
        ("Jsoup", "parse")  => "Jsoup.parse",
        ("Element", "select") => "select",
        ("Element", "selectFirst") => "selectFirst",
        ("Element", "text")  => "text",
        ("Element", "attr")  => "attr",
        ("Element", "html")  => "html",


        // Collections
        ("ArrayList", "add")   => "push",
        ("MutableList", "add") => "push",


        // Kotlin stdlib
        ("Regex", "find")       => "match",
        ("Regex", "findAll")    => "matchAll",
        ("Regex", "matches")    => "test",
        ("Regex", "containsMatchIn") => "test",
        ("MatchResult", "groupValues") => "/* groupValues */",

        // Kotlin Collections
        ("CollectionsKt", "toMutableList") => "Array.from",
        ("CollectionsKt", "addAll")                  => "push_all",
        ("CollectionsKt", "collectionSizeOrDefault") => "length",

        // Kotlin I/O & Streams
        ("CloseableKt",   "closeFinally")            => "_closeFinally",
        ("JvmStreamsKt",  "decodeFromStream")        => "_decodeFromStream",


        _ => return None,
    };
    Some(mapped.to_string())
}

pub fn kotlin_class_to_js(class: &str) -> String {
    match class {
        "java.lang.String"          => "String".into(),
        "java.lang.StringBuilder" => "StringBuilder".into(),
        "java.lang.Integer"         => "Number".into(),
        "java.lang.Boolean"         => "Boolean".into(),
        "kotlin.collections.ArrayList"
        | "java.util.ArrayList"     => "Array".into(),
        "okhttp3.HttpUrl"           => "HttpUrl".into(),
        "okhttp3.Request"         => "Request".into(),
        "okhttp3.FormBody"        => "FormBody".into(),
        "eu.kanade.tachiyomi.source.model.SManga" => "SManga".into(),
        "eu.kanade.tachiyomi.source.model.SChapter" => "SChapter".into(),
        "eu.kanade.tachiyomi.source.model.Page"   => "Page".into(),
        "eu.kanade.tachiyomi.source.model.MangasPage" => "MangasPage".into(),
        c => {
            // Return just the simple class name for unknown types
            c.split('.').last().unwrap_or(c).to_string()
        }
    }
}

pub fn from_dex_type(desc: &str) -> String {
    desc.trim_start_matches('L')
        .trim_end_matches(';')
        .replace('/', ".")
}

pub fn apply_well_known(js: &str) -> String {
    let mut s = js.to_string();


    let re_tostring = Regex::new(r#"\((\w+) \+ ""\)"#).unwrap();
    s = re_tostring.replace_all(&s, "String($1)").into_owned();


    s = s.replace("new Builder()", "new HttpUrlBuilder()");
    s = s.replace("HttpUrl.Builder()", "HttpUrl.Builder()");


    s = s.replace(".newCall(GET(", ".newCall(GET(");
    s = s.replace(".newCall(POST(", ".newCall(POST(");


    let re_static = Regex::new(r"/\* class \*/\._static(\d+)\(").unwrap();
    s = re_static.replace_all(&s, "/* static_meth$1 */(").into_owned();

    s
}