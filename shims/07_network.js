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

globalThis.NetworkHelper = function NetworkHelper() { return _networkHelper; };

globalThis.Request = class Request {
    constructor(url, method, headers, body, cacheControl) {
        this.url          = url?.toString?.() ?? url;
        this.method       = method ?? "GET";
        this.headers      = headers ?? new Headers();
        this.body         = body   ?? null;
        this.cacheControl = cacheControl ?? null;
    }

    static Builder = class RequestBuilder {
        constructor() {
            this._url     = null;
            this._method  = "GET";
            this._headers = new Headers();
            this._body    = null;
            this._cache   = null;
        }
        url(v)           { this._url    = v?.toString?.() ?? v; return this; }
        headers(v)       { this._headers = v; return this; }
        addHeader(k, v)  { this._headers.set(k, v);            return this; }
        cacheControl(v)  { this._cache  = v;                   return this; }
        get(body)        { this._method = "GET";    this._body = body ?? null; return this; }
        post(body)       { this._method = "POST";   this._body = body;         return this; }
        put(body)        { this._method = "PUT";    this._body = body;         return this; }
        delete(body)     { this._method = "DELETE"; this._body = body ?? null; return this; }
        build() {
            return new Request(this._url, this._method, this._headers, this._body, this._cache);
        }
    };
};

globalThis.Headers = class Headers {
    constructor(map = {}) {
        this._map = {};

        for (const k in map) {
            this._map[k.toLowerCase()] = map[k];
        }
    }

    get(name) {
        return this._map[name.toLowerCase()] ?? 0;
    }

    newBuilder() {
        const b = new Headers.Builder();
        for (const k in this._map) {
            b._map[k] = this._map[k];
        }
        return b;
    }

    set(name, value) {
        this._map[name.toLowerCase()] = String(value);
    }

    has(name) {
        return name.toLowerCase() in this._map;
    }

    delete(name) {
        delete this._map[name.toLowerCase()];
    }

    forEach(callback, thisArg = undefined) {
        for (const key in this._map) {
            callback.call(thisArg, this._map[key], key, this);
        }
    }

    entries() {
        return Object.entries(this._map)[Symbol.iterator]();
    }

    keys() {
        return Object.keys(this._map)[Symbol.iterator]();
    }

    values() {
        return Object.values(this._map)[Symbol.iterator]();
    }

    [Symbol.iterator]() {
        return this.entries();
    }

    toFetchHeaders() {
        return { ...this._map };
    }

    static Builder = class HeadersBuilder {
        constructor() {
            this._map = {};
        }

        push(name, value) {
            return this.add(name, value);
        }

        removeAll(name) {
            delete this._map[name.toLowerCase()];
            return this;
        }

        add(name, value) {
            this._map[name.toLowerCase()] = String(value);
            return this;
        }

        set(name, value) {
            this._map[name.toLowerCase()] = String(value);
            return this;
        }

        build() {
            return new Headers(this._map);
        }
    };
};
globalThis.Headers_Builder = Headers.Builder;

globalThis.HttpUrl = class HttpUrl {
    constructor(url) { this._url = url; }
    toString()   { return this._url; }
    fragment()   {
        const m = this._url.match(/#(.*)$/);
        return m ? m[1] : 0;
    }
    host()       { try { return new URL(this._url).hostname; } catch { return ""; } }
    encodedPath(){ try { return new URL(this._url).pathname; } catch { return "/"; } }
    newBuilder() { return new HttpUrl.Builder(this._url); }

    queryParameter(name) {
        try {
            return new URL(this._url).searchParams.get(name) ?? 0;
        } catch {
            return 0;
        }
    }

    pathSegments() {
        const makeList = (segs) => {
            segs.get = (i) => segs[i];
            segs.contains = (v) => segs.includes(v) ? 1 : 0;
            segs.isEmpty = () => segs.length === 0 ? 1 : 0;
            segs.size = () => segs.length;

            segs.iterator = () => {
                let i = 0;
                return {
                    hasNext: () => i < segs.length ? 1 : 0,
                    next: () => segs[i++]
                };
            };

            return segs;
        };

        try {
            return makeList(
                new URL(this._url).pathname
                    .split("/")
                    .filter(s => s.length > 0)
            );
        } catch {
            return makeList(
                this._url
                    .split("/")
                    .filter(s => s.length > 0)
            );
        }
    }

    static Builder = class HttpUrlBuilder {
        constructor(base = "") { this._url = base; }
        addQueryParameter(k, v) {
            if (v === null || v === undefined) return this;
            const sep = this._url.includes("?") ? "&" : "?";
            this._url += `${sep}${encodeURIComponent(k)}=${encodeURIComponent(v)}`;
            return this;
        }
        addPathSegment(v)  { this._url += `/${v}`; return this; }
        addPathSegments(v) {
            const parts = v.split("/").filter(p => p.length > 0);
            for (const part of parts) this._url += `/${part}`;
            return this;
        }
        removeAllQueryParameters(k) {
            try {
                const u = new URL(this._url);
                u.searchParams.delete(k);
                this._url = u.toString();
            } catch {}
            return this;
        }
        setQueryParameter(k, v) {
            try {
                const u = new URL(this._url);
                u.searchParams.set(k, v);
                this._url = u.toString();
            } catch {
                this.addQueryParameter(k, v);
            }
            return this;
        }
        fragment(v) { this._url += `#${v}`; return this; }
        build()     { return new HttpUrl(this._url); }
        toString()  { return this._url; }
        setEncodedQueryParameter(k, v) {
            try {
                const u = new URL(this._url);
                // delete all existing occurrences then re-add raw
                u.searchParams.delete(k);
                this._url = u.toString();
            } catch {}
            const sep = this._url.includes("?") ? "&" : "?";
            this._url += `${sep}${k}=${v}`;
            return this;
        }

        addEncodedQueryParameter(k, v) {
            if (v === null || v === undefined) return this;
            const sep = this._url.includes("?") ? "&" : "?";
            this._url += `${sep}${k}=${v}`;
            return this;
        }

        addEncodedPathSegments(v) {
            // v is already percent-encoded; append verbatim after stripping a
            // leading slash so we don't double-slash
            const segment = String(v).replace(/^\//, "");
            this._url = this._url.replace(/\/$/, "") + "/" + segment;
            return this;
        }
    };
};

HttpUrl.Companion = {
    get(url) {
        return new HttpUrl(url?.toString?.() ?? url);
    },

    parse(url) {
        if (!url) return new HttpUrl("");
        try {
            new URL(url.toString());
            return new HttpUrl(url.toString());
        } catch {
            return new HttpUrl("");
        }
    }
};

// String.toHttpUrl() extension
globalThis.toHttpUrl = (str) => new HttpUrl(str.toString());

globalThis.OkHttpClient = class OkHttpClient {
    constructor(useCloudflare = false) {
        this._useCloudflare = useCloudflare;
    }
    newCall(request) {
        return new globalThis._Call(request, false, this);
    }
};
globalThis._Call = class _Call {
    constructor(req, useCloudflare = false, clientInstance = null) {
        this._req = req;
        this._useCloudflare = useCloudflare;
        this._client = clientInstance;
    }

    execute() {
        let url = this._req.url?.toString?.() ?? String(this._req.url);
        const method = this._req.method ?? "GET";
        const headers = this._req.headers?.toFetchHeaders?.() ?? {};
        const body = _serializeBody(this._req.body ?? undefined);

        if ((!url || url === "") && globalThis.__lastExtractedUrl) {
            url = globalThis.__lastExtractedUrl;
        }
        globalThis.__lastExtractedUrl = "";

        const targetedDelay = this._client?._rateLimitDelay ?? 1000;

        const now = Date.now();
        const elapsed = now - (globalThis.__lastFetchTime ?? 0);

        if (elapsed < targetedDelay && globalThis.__lastFetchTime !== 0) {
            const sleepTime = targetedDelay - elapsed;
            if (typeof __native_sleep === 'function') {
                __native_sleep(sleepTime);
            }
        }
        globalThis.__lastFetchTime = Date.now();

        const result = fetchSync(url, { method, headers, body });
        if (result.cookies) {
            for (const [k, v] of Object.entries(result.cookies)) {
                _cookieStore.set(k, v);
            }
            state?.set?.("cookies", Object.fromEntries(_cookieStore));
        }
        return new _SandboxResponse(result.text, result.status, url);
    }
}

globalThis.firstInstance = function(iterator, predicate) {
    while (iterator.hasNext !== undefined ? iterator.hasNext() : false) {
        const item = iterator.next();
        if (predicate(item)) return item;
    }
    return 0;
};

globalThis.CacheControl_Builder = class CacheControl_Builder {
    constructor() {
        this._maxAgeSeconds = -1;
        this._noCache = false;
        this._noStore = false;
        this._onlyIfCached = false;
    }

    maxAge(value, unit) {
        // unit is a TimeUnit-like object or string; normalize to seconds
        if (unit && typeof unit.toSeconds === "function") {
            this._maxAgeSeconds = unit.toSeconds(value);
        } else if (typeof unit === "string") {
            switch (unit.toUpperCase()) {
                case "SECONDS":      this._maxAgeSeconds = value; break;
                case "MINUTES":      this._maxAgeSeconds = value * 60; break;
                case "HOURS":        this._maxAgeSeconds = value * 3600; break;
                case "DAYS":         this._maxAgeSeconds = value * 86400; break;
                default:             this._maxAgeSeconds = value; break;
            }
        } else {
            this._maxAgeSeconds = value;
        }
        return this;
    }

    noCache()      { this._noCache = true;      return this; }
    noStore()      { this._noStore = true;       return this; }
    onlyIfCached() { this._onlyIfCached = true;  return this; }

    build() {
        return {
            maxAgeSeconds: this._maxAgeSeconds,
            noCache:       this._noCache,
            noStore:       this._noStore,
            onlyIfCached:  this._onlyIfCached,
            toString() {
                const parts = [];
                if (this.maxAgeSeconds >= 0) parts.push(`max-age=${this.maxAgeSeconds}`);
                if (this.noCache)            parts.push("no-cache");
                if (this.noStore)            parts.push("no-store");
                if (this.onlyIfCached)       parts.push("only-if-cached");
                return parts.join(", ");
            },
        };
    }
}

globalThis.CacheControl = {
    FORCE_NETWORK: { noCache: true },
    FORCE_CACHE:   { onlyIfCached: true },
    Builder: CacheControl_Builder,
};

globalThis.RateLimitInterceptorKt = {
    rateLimit(builder, permits, period, timeUnit) {
        // Convert the given period into milliseconds using our TimeUnit helper
        let periodMs = 1000; // default fallback to 1 second
        if (timeUnit && typeof timeUnit.toMillis === 'function') {
            periodMs = timeUnit.toMillis(period);
        } else if (typeof period === 'number') {
            periodMs = period * 1000;
        }

        // Calculate a safe minimum spacing delay between individual requests
        const delayBetweenRequests = Math.ceil(periodMs / (permits || 1));

        // Inject the timing constraints into the custom client builder instance config
        if (builder) {
            builder._rateLimitDelay = delayBetweenRequests;
        }

        return builder;
    },

    // Handle standard structural compiler variants ($default variations)
    rateLimit$default(builder, permits, period, timeUnit, mask, obj) {
        // Handle Kotlin default arguments bitmask mapping
        if ((mask & 2) !== 0) period = 1;
        if ((mask & 4) !== 0) timeUnit = globalThis.TimeUnit?.SECONDS;

        return this.rateLimit(builder, permits, period, timeUnit);
    }
};

globalThis.RequestsKt = {
    GET(url, headers, cache) {
        if ((!url || url === "") && globalThis.__lastExtractedUrl) {
            url = globalThis.__lastExtractedUrl;
        }
        globalThis.__lastExtractedUrl = "";

        const h = typeof headers?.build === 'function' ? headers.build() : headers;
        return new Request.Builder().url(url).headers(h ?? new Headers()).cacheControl(cache ?? null).build();
    },
    GET$default(url, headers, cache, flags, mask) {
        if ((!url || url === "") && globalThis.__lastExtractedUrl) {
            url = globalThis.__lastExtractedUrl;
        }
        globalThis.__lastExtractedUrl = "";

        const h = typeof headers?.build === 'function' ? headers.build() : headers;
        return new Request.Builder().url(url).headers(h ?? new Headers()).cacheControl(cache ?? null).build();
    },
    POST$default(url, headers, body) {
        if ((!url || url === "") && globalThis.__lastExtractedUrl) {
            url = globalThis.__lastExtractedUrl;
        }
        globalThis.__lastExtractedUrl = "";

        const h = typeof headers?.build === 'function' ? headers.build() : headers;
        return new Request.Builder().url(url).headers(h ?? new Headers()).post(body).build();
    },

    POST(url, headers, body) {
        if ((!url || url === "") && globalThis.__lastExtractedUrl) {
            url = globalThis.__lastExtractedUrl;
        }
        globalThis.__lastExtractedUrl = "";

        const h = typeof headers?.build === 'function' ? headers.build() : headers;
        return new Request.Builder().url(url).headers(h ?? new Headers()).post(body).build();
    },
};

globalThis.GET  = (url, headers, cache) => RequestsKt.GET(url, headers, cache);
globalThis.POST = (url, headers, body, cache) =>
    new Request.Builder().url(url).headers(headers ?? new Headers()).post(body).build();

function _mergeCloudfareCookies(existingHeaders, cookies) {
    let out;
    if (existingHeaders instanceof Headers) {
        out = new Headers(existingHeaders._map); // copy the internal map
    } else {
        out = new Headers(existingHeaders ?? {});
    }

    if (cookies && cookies.length) {
        const cookieStr = cookies.map(c => `${c.name}=${c.value}`).join("; ");
        const existing = out.get("cookie") ?? "";
        out.set("cookie", existing ? `${existing}; ${cookieStr}` : cookieStr);
    }
    return out;
}

class BrotliInterceptor {}
globalThis.BrotliInterceptor = new BrotliInterceptor();

globalThis.okhttp3 = {
    brotli: { BrotliInterceptor: BrotliInterceptor },
};

globalThis.MediaType = {
    Companion: {
        get(str) { return { _type: str, toString() { return str; } }; },
        parse(str) { return { _type: str, toString() { return str; } }; },
    },
};

globalThis.FormBody_Builder = class FormBody_Builder {
    constructor() {
        this._params = [];
    }

    add(key, value) {
        this._params.push([String(key), String(value)]);
        return this;
    }

    push(key, value) {
        this._params.push([String(key), String(value)]);
        return this;
    }

    build() {
        return new FormBody(this._params);
    }
};

globalThis.FormBody = class FormBody {
    constructor(params = []) {
        this._params = params;
    }

    toRequestBody() {
        return this;
    }

    contentType() {
        return "application/x-www-form-urlencoded";
    }

    toString() {
        return this._params
            .map(([k, v]) =>
                encodeURIComponent(k) + "=" + encodeURIComponent(v))
            .join("&");
    }
};

globalThis.ResponseBody = class ResponseBody {
    constructor(text) { this._text = text; }
    source()  { return this; }
    string()  { return this._text; }
};

globalThis._cookieStore = new Map(Object.entries(state?.get?.("cookies") ?? {}));

globalThis._cfStateByOrigin = new Map(Object.entries(state?.get?.("cf_state") ?? {}));

globalThis._saveCfState = function _saveCfState() {
    state.set("cf_state", Object.fromEntries(_cfStateByOrigin));
}


globalThis._makeOkHttpClient = (useCloudflare, interceptors = [], networkInterceptors = []) => {
    const client = new OkHttpClient(useCloudflare);
    client._interceptors = interceptors;
    client._networkInterceptors = networkInterceptors;
    client.newBuilder = () => _makeOkHttpClientBuilder(useCloudflare, [...interceptors], [...networkInterceptors]);
    client.interceptors = () => _makeKotlinList(interceptors);
    client.networkInterceptors = () => _makeKotlinList(networkInterceptors);
    return client;
};
globalThis._makeOkHttpClientBuilder = (useCloudflare = false, interceptors = [], networkInterceptors = []) => ({
    _interceptors: interceptors,
    _networkInterceptors: networkInterceptors,
    _useCloudflare: useCloudflare,

    addInterceptor(i)        { this._interceptors.push(i);        return this; },
    addNetworkInterceptor(i) { this._networkInterceptors.push(i); return this; },
    interceptors()        { return _makeKotlinList(this._interceptors); },
    networkInterceptors() { return _makeKotlinList(this._networkInterceptors); },

    // builder fluent no-ops
    cookieJar(v)        { return this; },
    connectTimeout(...a){ return this; },
    readTimeout(...a)   { return this; },
    callTimeout(...a)   { return this; },
    cache(v)            { return this; },
    rateLimitHost(...a) { return this; },

    // DoH no-ops
    dohCloudflare()  { return this; },
    dohGoogle()      { return this; },
    dohAdGuard()     { return this; },
    dohQuad9()       { return this; },
    dohAliDNS()      { return this; },
    dohDNSPod()      { return this; },
    doh360()         { return this; },
    dohQuad101()     { return this; },
    dohMullvad()     { return this; },
    dohControlD()    { return this; },
    dohNajalla()     { return this; },
    dohShecan()      { return this; },

    apply(fn) { fn.call(this); return this; },

    build() {
        return _makeOkHttpClient(
            this._useCloudflare,
            [...this._interceptors],
            [...this._networkInterceptors]
        );
    }
});

globalThis._serializeBody = function _serializeBody(body) {
    if (body?._contentType) {
        return {
            body: body._body,
            contentType: body._contentType,
        };
    }

    if (body instanceof FormBody) {
        return {
            body: body.toString(),
            contentType: "application/x-www-form-urlencoded",
        };
    }

    return {
        body,
        contentType: null,
    };
}


if (globalThis.HttpSource) {
    HttpSource.prototype.getNetwork = function() { return _networkHelper; };
}
globalThis.getNetwork = () => _networkHelper;


globalThis.SpecificHostRateLimitInterceptorKt = {
    rateLimitHost(client, host, permits, period, unit, ...rest) {
        return client ?? rest[0] ?? { build() { return _makeOkHttpClient(false); } };
    },
};

globalThis._SandboxResponse = class _SandboxResponse {
    constructor(text, status, url) {
        this._text   = text;
        this._status = status;
        this._url    = url;
    }
    body()    { return new ResponseBody(this._text); }
    code()         { return this._status; }
    isSuccessful() { return this._status >= 200 && this._status < 300; }
    header(name) { return 0; }
    async text() { return this._text; }
    async json() { return JSON.parse(this._text); }

    request() {
        const url = this._url;
        return {
            url() { return new HttpUrl(url); },
            header(name) { return 0; },
            method() { return "GET"; },
        };
    }
    // asJsoup() comes later
}

globalThis.OkHttpExtensionsKt = {
    await(call, continuation) {
        const req = call._req;
        const useCloudflare = call._useCloudflare ?? false;
        const url     = req?.url?.toString?.() ?? String(req?.url ?? "");
        const method  = req?.method ?? "GET";
        const headers = req?.headers?.toFetchHeaders?.() ?? {};
        const body    = req?.body ?? undefined;

        const result = fetchSync(url, { method, headers, body });
        if (result.cookies) {
            for (const [k, v] of Object.entries(result.cookies)) {
                _cookieStore.set(k, v);
            }
            state?.set?.("cookies", Object.fromEntries(_cookieStore));
        }

        if (useCloudflare && (result.status === 403 || result.status === 503)) {
            if (typeof headless === "undefined" || !headless.available) {
                throw new Error(`Cloudflare challenge on ${url} but headless is not available`);
            }
            const cfResult = headless.fetchSync(url, { waitFor: "network_idle", block: ["images", "fonts"] });
            if (cfResult?.cookies?.length) {
                for (const c of cfResult.cookies) _cookieStore.set(c.name, c.value);
                state?.set?.("cookies", Object.fromEntries(_cookieStore));
            }
            const mergedHeaders = _mergeCloudfareCookies(headers, cfResult?.cookies ?? []);
            const retry = fetchSync(url, { method, headers: mergedHeaders, body });
            return new _SandboxResponse(retry.text, retry.status, url);
        }

        return new _SandboxResponse(result.text, result.status, url);
    }
};

globalThis.RequestBody = {
    Companion: {
        create(content, mediaType) {
            return {
                _body: content,
                _contentType: mediaType?._type ?? null,
            };
        }
    }
};