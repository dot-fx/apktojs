use crate::translator::dalvik::interpreter::JsExpr;
use crate::translator::resolver::infer::SymKey;
use crate::translator::resolver::pool::Pool;

pub fn is_resolved_method(method: &str, pool: &Pool, shard: usize) -> bool {
    if parse_meth_token(method).is_some() {
        return false;
    }

    if pool.methods.iter().any(|((s, _), m)| {
        *s == shard && m.js_name.as_deref() == Some(method)
    }) {
        return true;
    }

    matches!(method,
        "setUrl" | "setTitle" | "setAuthor" | "setArtist"
        | "setDescription" | "setGenre" | "setStatus"
        | "setThumbnail_url" | "setDate_upload" | "setChapter_number"
        | "setScanlator" | "setName" | "setImageUrl"
        | "collectionSizeOrDefault" | "toMutableList" | "addAll"
        | "joinToString" | "joinToString$default" | "iterator"
        | "hasNext" | "next" | "push" | "add" | "isEmpty" | "isNotEmpty"
        | "size" | "get" | "build" | "createListBuilder"
        | "isBlank" | "trim" | "append" | "toString" | "hashCode"
        | "equals" | "indexOf" | "parseBodyFragment" | "wholeText"
        | "newBuilder" | "addQueryParameter" | "fragment"
        | "body" | "source" | "request" | "url" | "execute"
        | "decodeFromBufferedSource" | "decodeFromString" | "serializer"
        | "tryParse" | "selectFirst" | "data" | "select" | "text" | "attr"
        | "getClass" | "getUrl" | "getBaseUrl" | "getHeaders" | "getClient" | "popularMangaRequest" | "popularMangaParse"
        | "searchMangaRequest" | "searchMangaParse"
        | "latestUpdatesRequest" | "latestUpdatesParse"
        | "mangaDetailsRequest" | "mangaDetailsParse"
        | "chapterListRequest" | "chapterListParse"
        | "pageListRequest" | "pageListParse"
        | "imageUrlRequest" | "imageUrlParse"
        | "imageRequest" | "headersBuilder"
        | "setUrlWithoutDomain" | "getMangaUrl" | "getChapterUrl"
        | "newCall" | "awaitSuccess" | "asObservableSuccess"
        | "addHeader" | "newCachelessCallWithProgress"
    )
}

pub fn obfuscated_call_key(expr: &JsExpr, pool: &Pool, shard: usize) -> Option<(SymKey, String)> {
    let JsExpr::MethodCall { method, .. } = expr else { return None };
    let (token_shard, idx) = parse_meth_token(method)?;
    if token_shard != shard { return None; }
    let m = pool.methods.get(&(shard, idx))?;
    if m.js_name.is_some() { return None; }

    if m.class_name.contains('.') { return None; }

    Some((SymKey::Method(shard, idx), m.class_name.clone()))
}

pub fn parse_meth_token(method: &str) -> Option<(usize, u32)> {
    let rest = method.strip_prefix("_meth")?;
    let (shard_str, idx_str) = rest.split_once('_')?;
    let shard = shard_str.parse::<usize>().ok()?;
    let idx   = idx_str.parse::<u32>().ok()?;
    Some((shard, idx))
}