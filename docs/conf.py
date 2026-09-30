"""Sphinx configuration for the unified PostProject documentation site."""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "python" / "src"))
sys.path.insert(0, str(Path(__file__).resolve().parent / "_ext"))

project = "PostProject"
author = "PostProject contributors"
release = "0.5.0-alpha.1"
version = "0.5"

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
html_title = f"PostProject {release} · ABI 36"
html_static_path = ["_static"]
html_css_files = ["postproject.css"]
templates_path = ["_templates"]
html_extra_path = ["CNAME", "versions.json"]
# Fonts and colors follow the landing page (https://www.postproject.org/). Its
# dark palette is used as-is; light mode keeps the fonts with a paper tone and
# a darker signal green that stays readable on a light background.
FONT_STACK = "Inter, ui-sans-serif, system-ui, sans-serif"
html_theme_options = {
    "source_repository": "https://github.com/postproject-org/postproject/",
    "source_branch": "main",
    "source_directory": "docs/",
    "light_css_variables": {
        "font-stack": FONT_STACK,
        "font-stack--headings": FONT_STACK,
        "font-stack--monospace": "ui-monospace, SFMono-Regular, Menlo, monospace",
        "color-foreground-primary": "#101211",
        "color-foreground-secondary": "#4a4f4b",
        "color-foreground-muted": "#6b706c",
        "color-foreground-border": "#c9c6bc",
        "color-background-primary": "#fbfaf6",
        "color-background-secondary": "#f2f0e9",
        "color-background-hover": "#e8e5dc",
        "color-background-border": "#dcd9cf",
        "color-brand-primary": "#101211",
        "color-brand-content": "#3b6a00",
        "color-brand-visited": "#3b6a00",
        "color-highlighted-background": "#e6f5cc",
        "color-inline-code-background": "#efece4",
    },
    "dark_css_variables": {
        "font-stack": FONT_STACK,
        "font-stack--headings": FONT_STACK,
        "font-stack--monospace": "ui-monospace, SFMono-Regular, Menlo, monospace",
        "color-foreground-primary": "#f5f1e8",
        "color-foreground-secondary": "#b8b4aa",
        "color-foreground-muted": "#8e8a82",
        "color-foreground-border": "#4a514b",
        "color-background-primary": "#101211",
        "color-background-secondary": "#191c1a",
        "color-background-hover": "#232724",
        "color-background-border": "#343a35",
        "color-brand-primary": "#f5f1e8",
        "color-brand-content": "#b9ff66",
        "color-brand-visited": "#b9ff66",
        "color-highlighted-background": "#33412b",
        "color-inline-code-background": "#191c1a",
        "color-code-background": "#191c1a",
    },
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
