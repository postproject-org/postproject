"""Sphinx configuration for the unified PostProject documentation site."""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "python" / "src"))
sys.path.insert(0, str(Path(__file__).resolve().parent / "_ext"))

project = "PostProject"
author = "PostProject contributors"
release = "0.7.0-alpha.1"
version = "0.7"

extensions = [
    "myst_parser",
    "breathe",
    "sphinx.ext.autodoc",
    "sphinx_design",
    "sphinxcontrib.mermaid",
    "postproject_code",
]
myst_enable_extensions = ["colon_fence", "deflist"]
# Plain ```mermaid fences render as diagrams here and on GitHub alike.
myst_fence_as_directive = ["mermaid"]
# Section headings up to level three get stable anchors for cross-page links.
myst_heading_anchors = 3
source_suffix = {".rst": "restructuredtext", ".md": "markdown"}
root_doc = "index"
exclude_patterns = ["_build", "_ext", "examples"]

# Tested programs whose marked regions populate the code-variants tabs.
postproject_code_examples = {
    "c": ["examples/c/*.c"],
    "cpp": ["examples/cpp/*.cpp"],
    "python": ["examples/python/*.py"],
    "rust": ["examples/rust/tests/*.rs"],
    "cli": ["examples/cli/*.sh"],
}

breathe_projects = {"PostProject": str(ROOT / "target" / "doxygen" / "xml")}
breathe_default_project = "PostProject"
breathe_domain_by_extension = {"h": "c", "hpp": "cpp"}

html_theme = "furo"
html_title = f"PostProject {release} · ABI 51"
html_static_path = ["_static"]
html_css_files = ["theme.css", "postproject.css"]
html_favicon = "_static/logo.svg"
templates_path = ["_templates"]
html_extra_path = ["CNAME", "versions.json"]
# theme.css and the licensed Inter font are shared with the landing page.
FONT_STACK = "Inter, ui-sans-serif, system-ui, sans-serif"
THEME_VARIABLES = {
    "font-stack": FONT_STACK,
    "font-stack--headings": FONT_STACK,
    "font-stack--monospace": "ui-monospace, SFMono-Regular, Menlo, monospace",
    "color-foreground-primary": "var(--ink)",
    "color-foreground-secondary": "var(--muted)",
    "color-foreground-muted": "var(--subtle)",
    "color-foreground-border": "var(--line)",
    "color-background-primary": "var(--paper)",
    "color-background-secondary": "var(--panel)",
    "color-background-hover": "var(--hover)",
    "color-background-border": "var(--line)",
    "color-brand-primary": "var(--ink)",
    "color-brand-content": "var(--signal)",
    "color-brand-visited": "var(--signal)",
    "color-highlighted-background": "var(--highlight)",
    "color-inline-code-background": "var(--code)",
    "color-code-background": "var(--panel)",
}
html_theme_options = {
    "source_repository": "https://github.com/postproject-org/postproject/",
    "source_branch": "main",
    "source_directory": "docs/",
    "light_css_variables": THEME_VARIABLES,
    "dark_css_variables": THEME_VARIABLES,
}
pygments_dark_style = "monokai"
# Diagrams size to their content and follow Furo's light/dark switch.
mermaid_version = "11.12.1"
mermaid_height = "auto"
mermaid_light_theme = "neutral"
mermaid_dark_theme = "dark"
mermaid_init_config = {
    "startOnLoad": False,
    "fontFamily": FONT_STACK,
    # Labels break where the source puts <br/>, not at Mermaid's narrow default.
    # A fixed pixel size lets the stylesheet shrink a wide diagram to fit but
    # never enlarge a small one.
    "flowchart": {"wrappingWidth": 400, "useMaxWidth": False},
    "sequence": {"useMaxWidth": False},
    "state": {"useMaxWidth": False},
}
html_context = {
    "landing_url": "https://www.postproject.org/",
    "landing_title": "postproject.org",
}
html_sidebars = {
    "**": [
        "landing-link.html",
        "sidebar/brand.html",
        "sidebar/search.html",
        "version-switcher.html",
        "language-switcher.html",
        "code-language-switcher.html",
        "sidebar/scroll-start.html",
        "sidebar/navigation.html",
        "sidebar/scroll-end.html",
    ]
}
