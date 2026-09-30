#!/usr/bin/env python3
"""images.json -> demo/assets/: Wikimedia Commons images for the fixture.

Only titles live in git; files are fetched on demand (1600 px thumbnails),
converted to the extension each fixture path asks for (ImageMagick), and
credited in assets/CREDITS.md from Commons' own license metadata.
Re-runs skip files already cached."""
import json, os, re, subprocess, sys, time, urllib.parse, urllib.request

S = os.path.dirname(os.path.abspath(__file__))
ASSETS = os.path.join(S, "assets")
UA = {"User-Agent": "tb-demo-fixture/0.1 (https://github.com/freakinfrick/treebeard; demo fixture)"}
WIDTH = 1600


def get(url, tries=5):
    for i in range(tries):
        try:
            return urllib.request.urlopen(urllib.request.Request(url, headers=UA), timeout=30).read()
        except urllib.error.HTTPError as e:
            if e.code != 429 or i == tries - 1:
                raise
            time.sleep(5 * (i + 1))  # Commons rate limit


def info(title):
    q = dict(action="query", titles=title, prop="imageinfo", iiprop="url|extmetadata", iiurlwidth=WIDTH,
             iiextmetadatafilter="Artist|LicenseShortName|LicenseUrl", format="json", formatversion=2)
    page = json.loads(get("https://commons.wikimedia.org/w/api.php?" + urllib.parse.urlencode(q)))["query"]["pages"][0]
    if "imageinfo" not in page:
        raise LookupError(f"not on Commons: {title}")
    ii = page["imageinfo"][0]
    meta = {k: re.sub(r"<[^>]+>", "", v["value"]).strip() for k, v in ii.get("extmetadata", {}).items()}
    return ii.get("thumburl") or ii["url"], ii["descriptionurl"], meta


def main():
    items = json.load(open(os.path.join(S, "images.json")))
    credits_path = os.path.join(ASSETS, "credits.json")
    credits = json.load(open(credits_path)) if os.path.exists(credits_path) else {}
    fails = 0
    for k, it in enumerate(items, 1):
        dest = os.path.join(ASSETS, it["path"])
        if os.path.exists(dest) and it["path"] in credits:
            continue
        try:
            url, page, meta = info(it["title"])
            raw = get(url)
            os.makedirs(os.path.dirname(dest), exist_ok=True)
            # [0]: first frame; convert picks the output format from dest's extension.
            subprocess.run(["convert", "-", dest], input=raw, check=True, capture_output=True)
            credits[it["path"]] = {"title": it["title"], "page": page, "artist": meta.get("Artist", "unknown"),
                                   "license": meta.get("LicenseShortName", "unknown"), "license_url": meta.get("LicenseUrl", "")}
            print(f"{k}/{len(items)} {it['path']}  [{credits[it['path']]['license']}]", flush=True)
        except Exception as e:  # keep going: a missing image just leaves a gap in the fixture
            fails += 1
            print(f"{k}/{len(items)} FAILED {it['path']}: {e}", file=sys.stderr, flush=True)
        time.sleep(1.5)
    os.makedirs(ASSETS, exist_ok=True)
    json.dump(credits, open(credits_path, "w"), indent=1, ensure_ascii=False)
    with open(os.path.join(ASSETS, "CREDITS.md"), "w") as f:
        f.write("# Image credits\n\nAll images from Wikimedia Commons, resized to 1600 px wide.\n\n")
        for path, c in sorted(credits.items()):
            lic = f"[{c['license']}]({c['license_url']})" if c["license_url"] else c["license"]
            f.write(f"- `{path}`: [{c['title'][5:]}]({c['page']}), {c['artist']}, {lic}\n")
    print(f"{len(credits)} cached, {fails} failed -> {ASSETS}")
    sys.exit(1 if fails else 0)


if __name__ == "__main__":
    main()
