"""Copy the project README into the MkDocs home page."""

from pathlib import Path


def on_config(config, **kwargs):
    root = Path(config.config_file_path).resolve().parent
    readme = root / "README.md"
    index = Path(config["docs_dir"]) / "index.md"

    if not readme.is_file():
        return config

    content = readme.read_text(encoding="utf-8")
    repo = config["repo_url"].rstrip("/")
    raw = repo.replace("https://github.com/", "https://raw.githubusercontent.com/") + "/master"
    content = content.replace("assets/", f"{raw}/assets/")
    content = content.replace("](LICENSE)", f"]({repo}/blob/master/LICENSE)")

    if not index.exists() or index.read_text(encoding="utf-8") != content:
        index.write_text(content, encoding="utf-8")

    return config
