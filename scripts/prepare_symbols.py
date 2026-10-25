"""Regenerate the vendored offline emoji catalogue from official Unicode/CLDR files."""
import json
from pathlib import Path
annotations = json.loads(Path("target/cldr-annotations-zh.json").read_text(encoding="utf-8"))["annotations"]["annotations"]
groups = {"Smileys & Emotion":"表情", "People & Body":"人物", "Component":"组件", "Animals & Nature":"自然", "Food & Drink":"食物", "Travel & Places":"旅行", "Activities":"活动", "Objects":"物品", "Symbols":"符号", "Flags":"旗帜"}
result = []
group = ""
for line in Path("target/emoji-test.txt").read_text(encoding="utf-8").splitlines():
    if line.startswith("# group:"):
        group = groups[line[8:].strip()]
    elif "; fully-qualified" in line:
        codes, description = line.split("#", 1)
        symbol = "".join(chr(int(v,16)) for v in codes.split(";")[0].split())
        fallback = description.strip().split(" ",2)[-1]
        entry = annotations.get(symbol, annotations.get(symbol.replace("\ufe0f",""), {}))
        name = entry.get("tts", [fallback])[0]
        keywords = " ".join(entry.get("default", [name]))
        result.append(dict(symbol=symbol,name=name,keywords=keywords,group=group))
Path("assets/emoji-zh.json").write_text(json.dumps(result,ensure_ascii=False,separators=(",",":")),encoding="utf-8")
print(f"Wrote {len(result)} offline emoji entries")
