#!/usr/bin/env python3
"""Generate static HTML pages for the Key-Bitcher docs site.

Reads the markdown sources in this folder (front matter + body), converts them
to themed HTML pages, and writes <name>.html next to the sources.

Usage:  python docs/_build.py
Deps:   pip install markdown
"""

import re
import sys
from pathlib import Path

import markdown

DOCS = Path(__file__).parent

PAGES = [
    ("index.md", "Home", "index.html", "active-index"),
    ("getting-started.md", "Getting Started", "getting-started.html", "active-getting-started"),
    ("commands.md", "Commands", "commands.html", "active-commands"),
    ("configuration.md", "Configuration", "configuration.html", "active-configuration"),
    ("wave-integration.md", "Wave Integration", "wave-integration.html", "active-wave-integration"),
    ("architecture.md", "Architecture", "architecture.html", "active-architecture"),
    ("security.md", "Security", "security.html", "active-security"),
    ("ide-integrations.md", "IDE & Terminal Integrations", "ide-integrations.html", "active-ide-integrations"),
    ("development.md", "Development", "development.html", "active-development"),
]

NAV = """
<nav class="nav-menu">
%s
</nav>
"""


def front_matter_and_body(text):
    """Return (title, body). title from YAML front matter if present."""
    m = re.match(r"\A---\r?\n(.*?)\r?\n---\r?\n", text, re.DOTALL)
    title = None
    if m:
        fm = m.group(1)
        t = re.search(r"^title:\s*(.+)$", fm, re.MULTILINE)
        if t:
            title = t.group(1).strip()
        text = text[m.end():]
    return title, text


def normalize(text):
    """Apply Key-Bitcher renaming and internal link rewrites."""
    text = text.replace("merlin-tribukait/ai-env-plugin", "merlin-tribukait/KEY-BITCHER")
    text = text.replace("https://merlin-tribukait.github.io/ai-env-plugin",
                        "https://merlin-tribukait.github.io/KEY-BITCHER/")
    text = text.replace("plugin_config.toml", "key-bitcher.toml")
    text = text.replace("ai-env-plugin", "key-bitcher")
    text = text.replace("ai-env", "key-bitcher")
    for src, _, dst, _ in PAGES:
        stem = src[:-3]
        # [x](stem) -> [x](dst)
        text = re.sub(r"\]\(\s*" + re.escape(stem) + r"\s*\)", "](" + dst + ")", text)
    text = text.replace("(../SECURITY.md)", "(security.html)")
    text = text.replace("(../README.md)", "(index.html)")
    return text


def render_html(title, body_html, active_class):
    links = []
    for _, label, href, cls in PAGES:
        active = " active" if cls == active_class else ""
        links.append('        <a href="%s" class="nav-link%s">%s</a>' % (href, active, label))
    nav = NAV % "\n".join(links)
    quote = ('  <div class="bitch-quote doc-quote reveal" aria-hidden="true">'
             '<span class="bq-mark">\U0001F4AC</span>'
             '<p>Error message #42: "Your .env has opinions. Your .env is wrong."</p></div>')
    body_html = re.sub(r"(<h1>[^<]*</h1>)", r"\1\n" + quote, body_html, count=1)
    return TEMPLATE.replace("{nav}", nav).replace("{title}", title).replace("{body}", body_html)


TEMPLATE = """<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{title} | Key-Bitcher</title>
  <link rel="stylesheet" href="styles.css">
  <link rel="stylesheet" href="docs.css">
  <link rel="icon" href="assets/logo-badge.svg">
</head>
<body>
  <div class="top-banner">
    <button class="mobile-menu-btn" aria-label="Open menu" aria-expanded="false"><span></span><span></span><span></span></button>
    <span>&#9888;&#65039; WARNING: Key-Bitcher strictly labels developer incompetence. Do not ask for support.</span>
    <div class="banner-actions">
      <a href="key-goblin.html" class="banner-goblin-link" title="Switch to the Key-Goblin Hoard">&#129421; KEY-GOBLIN HOARD &#8599;</a>
      <div class="bitch-toggle-wrap">
        <span class="toggle-label">BITCH MODE:</span>
        <label class="switch">
          <input type="checkbox" id="bitchModeToggle">
          <span class="slider round"></span>
        </label>
        <span id="bitchStatus" class="status-off">OFF</span>
      </div>
    </div>
  </div>

  <div class="app-layout">
    <aside class="sidebar">
      <div class="brand">
        <a href="index.html"><img src="assets/logo-badge.svg" alt="Key-Bitcher Logo" class="brand-img"></a>
        <h2>KEY-BITCHER</h2>
        <p class="tagline">Managing your keys. No apologies.</p>
      </div>
      {nav}
      <div class="nav-group-label">Roadmap</div>
      <nav class="nav-menu">
        <a href="key-goblin.html" class="nav-link">Key-Goblin Hoard &#129421;</a>
      </nav>
      <div class="repo-box">
        <p>Repo target:</p>
        <code>merlin-tribukait/KEY-BITCHER</code>
        <a href="https://github.com/merlin-tribukait/KEY-BITCHER" target="_blank" class="btn-git">View on GitHub &#8599;</a>
      </div>
    </aside>
    <main class="content docs-content">
      {body}
    </main>
  </div>
  <script src="toggle.js"></script>
  <script src="effects.js"></script>
</body>
</html>
"""

MD = markdown.Markdown(extensions=["tables", "fenced_code", "sane_lists", "nl2br", "attr_list"])


def main():
    for src, default_title, dst, cls in PAGES:
        if dst == "index.html":
            continue  # index.html is the hand-written interactive landing page
        path = DOCS / src
        if not path.exists():
            print(f"skip missing {src}")
            continue
        text = path.read_text(encoding="utf-8")
        title, body = front_matter_and_body(text)
        title = title or default_title
        body = normalize(body)
        body_html = MD.convert(body)
        MD.reset()
        html = render_html(title, body_html, cls)
        (DOCS / dst).write_text(html, encoding="utf-8")
        print(f"wrote {dst} ({title})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
