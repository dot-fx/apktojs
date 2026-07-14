globalThis._JsoupNull = new Proxy({}, {
    get(_, prop) {
        if (prop === "then")              return undefined;          // not a Promise
        if (prop === Symbol.toPrimitive)  return () => "";
        if (prop === Symbol.iterator)     return [][Symbol.iterator].bind([]);
        if (prop === "toString")          return () => "";
        if (prop === "_isJsoupNull")      return true;
        return (...args) => _JsoupNull;
    }
});


function _resolveUrl(val, baseUri) {
    if (!val) return "";
    val = val.trim();
    if (!val) return "";
    if (/^https?:\/\//i.test(val)) return val;
    if (!baseUri) return val;
    try { return new URL(val, baseUri).href; } catch { return val; }
}

function _queryHtml(html, selector) {
    if (!html || !selector) return [];
    try {
        const raw = JSON.parse(__native_html_query(html, selector));
        if (raw.error) { console.warn("[jsoup] query error:", raw.error); return []; }
        return raw; // array of {text, html, outer, attrs, own_text, tag}
    } catch (e) {
        console.warn("[jsoup] _queryHtml threw:", e?.message ?? e);
        return [];
    }
}


globalThis.parseHTML = function(html) {
    return function $(selector) {
        const items = _queryHtml(html, selector);

        items.attr  = function(name) { return this.length > 0 ? (this[0].attrs?.[name] ?? null) : null; };
        items.text  = function()     { return this.map(r => r.text ?? "").join(""); };
        items.html  = function()     { return this.length > 0 ? (this[0].html ?? null) : null; };
        items.each  = function(fn)   { this.forEach((item, i) => fn(i, item)); return this; };
        items.find  = function(sel)  { return this.length > 0 ? $(sel) : Object.assign([], { attr: () => null, text: () => "", html: () => null, each: () => items, find: () => items }); };

        return items;
    };
};


globalThis.JsoupDocument = class JsoupDocument {
    constructor(html, baseUri = "") {
        this._html    = html ?? "";
        this._baseUri = baseUri || "";
    }

    _query(selector) {
        return _queryHtml(this._html, selector);
    }

    select(selector) {
        if (!selector) return new JsoupElements([], this._baseUri, this._html);
        return new JsoupElements(this._query(selector), this._baseUri, this._html);
    }

    selectFirst(selector) {
        if (!selector) return _JsoupNull;
        return this.select(selector).first();
    }

    text() {
        const items = this._query("body");
        return items[0]?.text ?? "";
    }

    html()      { return this._html; }
    outerHtml() { return this._html; }

    title() {
        const items = this._query("title");
        return items[0]?.text ?? "";
    }

    location()  { return this._baseUri; }
    wholeText() { return this.text(); }

    body()        { return this.select("body").first(); }
    head()        { return this.select("head").first(); }

    getElementById(id)      { return this.select(`#${id}`).first(); }
    getElementsByTag(tag)   { return this.select(tag); }
    getElementsByClass(cls) { return this.select(`.${cls}`); }

    absUrl(attributeKey) {
        return this.body()?.absUrl?.(attributeKey) ?? "";
    }
};

globalThis.JsoupElements = class JsoupElements {

    constructor(rawItems, baseUri = "", contextHtml = "") {
        this._baseUri     = baseUri;
        this._contextHtml = contextHtml;
        // Accept either raw item arrays or pre-built JsoupElement arrays
        this._els = (rawItems ?? []).map(item =>
            item instanceof JsoupElement
                ? item
                : new JsoupElement(item, baseUri, item.html ?? contextHtml)
        );
    }

    size()    { return this._els.length; }
    isEmpty() { return this._els.length === 0; }

    get length()     { return this._els.length; }
    get length_val() { return this._els.length; }
    get size_val()   { return this._els.length; }

    first() { return this._els.length ? this._els[0]                        : _JsoupNull; }
    last()  { return this._els.length ? this._els[this._els.length - 1]     : _JsoupNull; }
    get(i)  { return this._els[i]     ?? _JsoupNull; }

    location() { return this._baseUri; }


    text()      { return this._els.map(el => el.text()).join(" ").trim(); }
    ownText()   { return this._els[0]?.ownText()   ?? ""; }
    wholeText() { return this._els.map(el => el.wholeText()).join("").trim(); }
    html()      { return this._els[0]?.html()      ?? ""; }
    outerHtml() { return this._els[0]?.outerHtml() ?? ""; }
    data()      { return this._els[0]?.data()      ?? ""; }
    eachText()  { return this._els.map(el => el.text()); }

    attr(name)    { return this._els[0]?.attr(name)    ?? ""; }
    hasAttr(name) { return this._els[0]?.hasAttr(name) ?? 0;  }
    absUrl(key)   { return this._els[0]?.absUrl(key)   ?? ""; }


    select(selector) {
        if (!this._els.length) return new JsoupElements([], this._baseUri, this._contextHtml);
        // Search within first element's subtree (matches Jsoup's .select() on Elements)
        return this._els[0].select(selector);
    }

    selectFirst(selector) {
        if (!selector) return _JsoupNull;
        return this.select(selector).first();
    }

    forEach(fn)  { this._els.forEach(fn); }
    map(fn)      { return this._els.map(fn); }
    filter(fn)   { return this._els.filter(fn); }

    [Symbol.iterator]() { return this._els[Symbol.iterator](); }
};

globalThis.JsoupElement = class JsoupElement {
    constructor(raw, baseUri = "", contextHtml = "") {
        this._raw         = raw ?? {};
        this._baseUri     = baseUri;
        this._contextHtml = raw?.html ?? raw?.outer ?? contextHtml ?? "";
    }


    text()      { return this._raw.text      ?? ""; }
    ownText()   { return this._raw.own_text  ?? ""; }
    wholeText() { return this._raw.text      ?? ""; }  // Rust already collects all text nodes
    html()      { return this._raw.html      ?? ""; }
    outerHtml() { return this._raw.outer     ?? ""; }
    data()      { return this._raw.text      ?? ""; }  // for <script>/<style> content


    attr(name) {
        if (!name) return "";

        if (name.startsWith("abs:")) {
            let realAttr = name.slice(4);
            if (realAttr === "img") realAttr = "src";   // translator-bug workaround
            const val = this._raw.attrs?.[realAttr] ?? null;
            return _resolveUrl(val, this._baseUri);
        }

        return this._raw.attrs?.[name] ?? "";
    }

    hasAttr(name) {
        if (!name) return 0;
        return (name in (this._raw.attrs ?? {})) ? 1 : 0;
    }

    id()        { return this._raw.attrs?.id    ?? ""; }
    className() { return this._raw.attrs?.class ?? ""; }
    tagName()   { return this._raw.tag          ?? ""; }

    absUrl(attributeKey) {
        const raw = this.attr(attributeKey);
        if (!raw) return "";
        const full = _resolveUrl(raw, this._baseUri);
        if (attributeKey === "href" || attributeKey === "src") {
            globalThis.__lastExtractedUrl = full;
        }
        return full;
    }

    select(selector) {
        if (!selector || !this._contextHtml) {
            return new JsoupElements([], this._baseUri, this._contextHtml);
        }
        // Search within this element's inner HTML subtree
        const items = _queryHtml(this._contextHtml, selector);
        return new JsoupElements(items, this._baseUri, this._contextHtml);
    }

    selectFirst(selector) {
        if (!selector) return _JsoupNull;
        return this.select(selector).first();
    }

    eachText() {
        const items = _queryHtml(this._contextHtml, ":scope > *");
        return items.map(i => i.text ?? "");
    }

    toString() { return this.outerHtml(); }
};

globalThis["JsoupExtensionsKt"] = {
    ["asJsoup$default"](body, baseUri, charset, flags) {
        const html = typeof body?.string === "function"
            ? body.string()
            : (body?._text ?? "");
        return new JsoupDocument(html, baseUri ?? "");
    },
    ["asJsoup"](body, baseUri, charset) {
        const html = typeof body?.string === "function"
            ? body.string()
            : (body?._text ?? "");
        return new JsoupDocument(html, baseUri ?? "");
    },
};

globalThis.Jsoup = {
    parseBodyFragment(html, baseUri = "") {
        return new JsoupDocument(html, baseUri);
    },
    parse(html, baseUri = "") {
        return new JsoupDocument(html, baseUri);
    },
    connect(url) {
        return {
            get()      { return this; },
            post()     { return this; },
            header()   { return this; },
            timeout()  { return this; },
            execute()  { return { parse() { return new JsoupDocument(""); } }; },
        };
    },
};

const _networkHelper = {
    cookieJar: {
        saveFromResponse(url, cookies) {
            for (const c of cookies) _cookieStore.set(c.name, c.value);
            state?.set?.("cookies", Object.fromEntries(_cookieStore));
        },
        loadForRequest(url) { return []; },
    },

    get client()             { return _makeOkHttpClient(true);  },
    get nonCloudflareClient(){ return _makeOkHttpClient(false); },
    get cloudflareClient()   { return _makeOkHttpClient(true);  },
};