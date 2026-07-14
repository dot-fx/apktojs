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
        return raw;
    } catch (e) {
        console.warn("[jsoup] _queryHtml threw:", e?.message ?? e);
        return [];
    }
}

// Unified Jquery-like selector API
globalThis.parseHTML = function(html) {
    return function $(selector) {
        const items = _queryHtml(html, selector);
        const wrapped = items.map(item => {
            const el = new JsoupElement(item, "", html);
            return {
                text:  ()    => el.text(),
                html:  ()    => el.html(),
                outer: ()    => el.outerHtml(),
                attr:  (name) => el.attr(name),
                find:  (sel)  => parseHTML(el.html())(sel),
                _raw: item,
            };
        });

        wrapped.attr = function(name) { return this.length > 0 ? this[0].attr(name) : null; };
        wrapped.text = function()     { return this.map(r => r.text()).join(""); };
        wrapped.html = function()     { return this.length > 0 ? this[0].html() : null; };
        wrapped.each = function(fn)   { this.forEach((item, i) => fn(i, item)); return this; };
        wrapped.find = function(sel)  { return this.length > 0 ? parseHTML(this[0].html())(sel) : wrapped; };

        return wrapped;
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
        if (!selector) return 0;
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

    first() { return this._els.length ? this._els[0]                        : 0; }
    last()  { return this._els.length ? this._els[this._els.length - 1]     : 0; }
    get(i)  { return this._els[i]     ?? 0; }

    eq(index) {
        const el = this.get(index);
        return el !== 0 ? new JsoupElements([el], this._baseUri, this._contextHtml) : new JsoupElements([], this._baseUri, this._contextHtml);
    }

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

    hasClass(className) {
        return this._els.some(el => el.hasClass(className));
    }

    select(selector) {
        if (!selector || !this._els.length) {
            return new JsoupElements([], this._baseUri, this._contextHtml);
        }
        // Jsoup spec: Collection selector aggregates results from all children
        let results = [];
        for (const el of this._els) {
            results.push(..._queryHtml(el.html(), selector));
        }
        return new JsoupElements(results, this._baseUri, this._contextHtml);
    }

    selectFirst(selector) {
        if (!selector) return 0;
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
    wholeText() { return this._raw.text      ?? ""; }
    html()      { return this._raw.html      ?? ""; }
    outerHtml() { return this._raw.outer     ?? ""; }
    data()      { return this._raw.text      ?? ""; }

    attr(name) {
        if (!name) return "";
        if (name.startsWith("abs:")) {
            let realAttr = name.slice(4);
            if (realAttr === "img") realAttr = "src";
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

    hasClass(className) {
        const cls = this.className();
        if (!cls) return false;
        return cls.split(/\s+/).includes(className);
    }

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
        const items = _queryHtml(this._contextHtml, selector);
        return new JsoupElements(items, this._baseUri, this._contextHtml);
    }

    selectFirst(selector) {
        if (!selector) return 0;
        return this.select(selector).first();
    }

    // Resolves parent dynamically using contextual contains lookup
    parent() {
        if (!this._contextHtml) return 0;
        const outer = this.outerHtml();
        if (!outer) return 0;

        const all = _queryHtml(this._contextHtml, "*");
        for (const item of all) {
            if (item.html && item.html.includes(outer) && item.outer !== outer) {
                return new JsoupElement(item, this._baseUri, this._contextHtml);
            }
        }
        return 0;
    }

    // Advanced flat AST scanning to determine direct children nodes
    children() {
        const descendants = _queryHtml(this.html(), "*");
        let childrenRaw = [];
        let i = 0;
        while (i < descendants.length) {
            const current = descendants[i];
            childrenRaw.push(current);
            const currentOuter = current.outer;
            i++;
            while (i < descendants.length && currentOuter.includes(descendants[i].outer)) {
                i++;
            }
        }
        return new JsoupElements(childrenRaw, this._baseUri, this.html());
    }

    nextElementSibling() {
        const parent = this.parent();
        if (parent === 0) return 0;
        const siblings = parent.children();
        const outer = this.outerHtml();
        for (let i = 0; i < siblings.length; i++) {
            if (siblings.get(i).outerHtml() === outer) {
                return siblings.get(i + 1);
            }
        }
        return 0;
    }

    previousElementSibling() {
        const parent = this.parent();
        if (parent === 0) return 0;
        const siblings = parent.children();
        const outer = this.outerHtml();
        for (let i = 0; i < siblings.length; i++) {
            if (siblings.get(i).outerHtml() === outer) {
                return i > 0 ? siblings.get(i - 1) : 0;
            }
        }
        return 0;
    }

    eachText() {
        return this.children().map(i => i.text() ?? "");
    }

    toString() { return this.outerHtml(); }
};

globalThis.Evaluator = class Evaluator {
    matches(root, element) { return false; }
};

globalThis.Evaluator_Tag = class Evaluator_Tag extends globalThis.Evaluator {
    constructor(tagName) {
        super();
        this.tagName = tagName.toLowerCase();
    }
    matches(root, element) {
        return element.tagName().toLowerCase() === this.tagName;
    }
};

globalThis.Evaluator_Id = class Evaluator_Id extends globalThis.Evaluator {
    constructor(id) {
        super();
        this.id = id;
    }
    matches(root, element) {
        return element.id() === this.id;
    }
};

globalThis.Evaluator_Class = class Evaluator_Class extends globalThis.Evaluator {
    constructor(className) {
        super();
        this.className = className.toLowerCase();
    }
    matches(root, element) {
        return element.className().toLowerCase().split(/\s+/).includes(this.className);
    }
};

globalThis["JsoupExtensionsKt"] = {
    ["asJsoup$default"](body, baseUri, charset, flags) {
        const html = typeof body?.string === "function" ? body.string() : (body?._text ?? "");
        return new JsoupDocument(html, baseUri ?? "");
    },
    ["asJsoup"](body, baseUri, charset) {
        const html = typeof body?.string === "function" ? body.string() : (body?._text ?? "");
        return new JsoupDocument(html, baseUri ?? "");
    },
};

globalThis.Jsoup = {
    parseBodyFragment(html, baseUri = "") { return new JsoupDocument(html, baseUri); },
    parse(html, baseUri = "") { return new JsoupDocument(html, baseUri); },
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