import argparse
import asyncio
import json
import os
import re
import sys
import urllib.parse
from playwright.async_api import async_playwright

def parse_citations(page_content, inner_text):
    citations = []
    seen = set()
    link_regex = re.compile(r'href="(https?://[^"]+)"[^>]*data-ved=[^>]*>.*?<span[^>]*>([^<]+)</span>', re.DOTALL)
    for m in link_regex.finditer(page_content):
        href = m.group(1)
        name = m.group(2).strip()
        if "google.com" in href or href in seen or not name:
            continue
        seen.add(href)
        citations.append({"title": name, "url": href})
    return citations

def extract_clean_text(inner_text):
    lines = [line.strip() for line in inner_text.splitlines() if line.strip()]
    cleaned = []
    for line in lines:
        if "About this page" in line or "unusual traffic" in line:
            return ""
        if "Search Labs" in line or "Feedback" in line or "Generative AI is experimental" in line:
            continue
        cleaned.append(line)
    return "\n".join(cleaned)

def format_prompt(query, mode):
    q = query.strip()
    if mode == "refine":
        if "what would the fixed text look like?" in q:
            return q
        return f" what would the fixed text look like?\n{q}"
    elif mode == "research":
        return f"Направи задълбочено проучване с факти, контекст и източници за следната тема или лице: {q}"
    elif mode == "factcheck":
        return f"Провери истинността на следното твърдение, анализирай надеждни източници и изведи факти и заключение:\n{q}"
    return q

async def fetch_ai_mode(query, mode, session_dir, headless, json_output, wait_seconds):
    os.makedirs(session_dir, exist_ok=True)
    prompt = format_prompt(query, mode)

    async with async_playwright() as p:
        try:
            browser = await p.chromium.launch_persistent_context(
                user_data_dir=os.path.abspath(session_dir),
                channel="msedge",
                headless=headless,
                args=[
                    "--disable-blink-features=AutomationControlled",
                    "--no-sandbox"
                ],
                user_agent="Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36"
            )
        except Exception:
            browser = await p.chromium.launch_persistent_context(
                user_data_dir=os.path.abspath(session_dir),
                headless=headless,
                args=[
                    "--disable-blink-features=AutomationControlled",
                    "--no-sandbox"
                ],
                user_agent="Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36"
            )

        page = await browser.new_page()
        encoded = urllib.parse.quote(prompt)
        url = f"https://www.google.com/search?udm=50&q={encoded}"

        await page.goto(url, wait_until="domcontentloaded")

        consent_selectors = [
            "#L2AGLb",
            "button:has-text('Приемам')",
            "button:has-text('Accept all')",
            "button:has-text('I agree')",
            "button:has-text('Accept')"
        ]
        for sel in consent_selectors:
            try:
                b = page.locator(sel)
                if await b.count() > 0:
                    await b.first.click()
                    await page.wait_for_timeout(1000)
                    break
            except Exception:
                pass

        await page.wait_for_timeout(wait_seconds * 1000)

        html = await page.content()
        body_text = await page.evaluate("() => document.body.innerText")

        if "unusual traffic" in body_text or "captcha" in html.lower():
            if not headless:
                print("CAPTCHA detected. Please solve it in the browser window...", file=sys.stderr)
                await page.wait_for_timeout(25000)
                html = await page.content()
                body_text = await page.evaluate("() => document.body.innerText")
            else:
                res = {
                    "ok": False,
                    "error": "captcha_required",
                    "message": "Google CAPTCHA detected. Run with --login or --headful to solve it in the session context.",
                    "query": query,
                    "mode": mode
                }
                if json_output:
                    print(json.dumps(res, ensure_ascii=False))
                else:
                    print(f"Error: {res['message']}", file=sys.stderr)
                await browser.close()
                return res

        extracted = ""
        selectors = [
            "div[data-asf='ai-mode-response']",
            "div[data-processed='true']",
            "[data-attrid='wa:/description']",
            "#rso"
        ]
        for sel in selectors:
            loc = page.locator(sel)
            if await loc.count() > 0:
                raw = await loc.first.inner_text()
                cand = extract_clean_text(raw)
                if len(cand) > len(extracted):
                    extracted = cand

        if not extracted:
            q_match = re.search(r'[„"“]([А-ЯA-Z][^"“”]{15,})[“"”]', html)
            if q_match:
                extracted = q_match.group(1).strip()
            else:
                extracted = extract_clean_text(body_text)

        citations = parse_citations(html, extracted)

        await browser.close()

        res = {
            "ok": True,
            "query": query,
            "mode": mode,
            "text": extracted,
            "citations": citations
        }

        if json_output:
            print(json.dumps(res, ensure_ascii=False, indent=2))
        else:
            print(extracted)
            if citations:
                print("\nCitations & Sources:")
                for c in citations:
                    print(f"- {c['title']}: {c['url']}")

        return res

def main():
    parser = argparse.ArgumentParser(description="Query Google AI Mode search (udm=50) via terminal")
    parser.add_argument("query", nargs="?", default="", help="Query or text to process")
    parser.add_argument("--query", "-q", dest="opt_query", default="", help="Query or text to process")
    parser.add_argument("--mode", choices=["refine", "research", "factcheck", "raw"], default="raw", help="Processing mode")
    parser.add_argument("--login", "--headful", dest="headful", action="store_true", help="Launch visible browser to login or solve CAPTCHA")
    parser.add_argument("--session-dir", default="data/google_session", help="Path to persistent browser profile")
    parser.add_argument("--json", action="store_true", help="Output JSON format")
    parser.add_argument("--wait", type=int, default=4, help="Seconds to wait for dynamic response streaming")

    args = parser.parse_args()
    q = args.query or args.opt_query
    if not q:
        if args.headful:
            q = "Google AI Search"
        else:
            parser.print_help()
            sys.exit(1)

    asyncio.run(fetch_ai_mode(
        query=q,
        mode=args.mode,
        session_dir=args.session_dir,
        headless=not args.headful,
        json_output=args.json,
        wait_seconds=args.wait
    ))

if __name__ == "__main__":
    main()
