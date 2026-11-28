"""Copy the project README into the MkDocs home page."""

from pathlib import Path

_SENTENCE_HEADINGS = (
    "### octorun is a tool that lets you",
    "### octorun works on all the platforms",
    "### If you want to create new empty runners",
)


def _docs_markdown(readme: str) -> str:
    lines = []
    first_h1 = True
    in_code = False

    for line in readme.splitlines():
        if line.startswith("```"):
            in_code = not in_code
            lines.append(line)
            continue
        if in_code:
            lines.append(line)
            continue

        if line.startswith(_SENTENCE_HEADINGS):
            lines.append(line[4:])
        elif line.startswith("# "):
            if first_h1:
                first_h1 = False
                lines.append(line)
            else:
                lines.append(f"## {line[2:]}")
        elif line.startswith("## "):
            lines.append(f"### {line[3:]}")
        elif line.startswith("### "):
            lines.append(f"#### {line[4:]}")
        else:
            lines.append(line)

    return "\n".join(lines)


def on_config(config, **kwargs):
    root = Path(config.config_file_path).resolve().parent
    readme = root / "README.md"
    index = Path(config["docs_dir"]) / "index.md"

    if not readme.is_file():
        return config

    content = _docs_markdown(readme.read_text(encoding="utf-8"))
    repo = config["repo_url"].rstrip("/")
    raw = (
        repo.replace("https://github.com/", "https://raw.githubusercontent.com/")
        + "/master"
    )
    content = content.replace("assets/", f"{raw}/assets/")
    content = content.replace("](LICENSE)", f"]({repo}/blob/master/LICENSE)")

    if not index.exists() or index.read_text(encoding="utf-8") != content:
        index.write_text(content, encoding="utf-8")

    return config
