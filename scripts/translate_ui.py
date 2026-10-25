"""Development-only translation draft generator; only static UI literals are sent.
The application embeds the resulting JSON and never calls a translation service.
"""
import concurrent.futures
import json
import re
import urllib.parse
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LOCALES = ['zh-CN', 'zh-TW', 'en', 'ja', 'ko', 'es', 'fr', 'de', 'pt']
keys = {'语言', '界面语言', '语言切换后立即生效并自动保存。', '清空搜索'}
for path in [ROOT / 'src/ui.rs', *(ROOT / 'src/ui').rglob('*.rs'), ROOT / 'src/service.rs', ROOT / 'src/preview.rs', *(ROOT / 'src/platform').glob('*.rs')]:
    if path.name == 'review.rs':
        continue
    source = path.read_text(encoding='utf-8').split('#[cfg(test)]')[0]
    for match in re.finditer(r'"(?:[^"\\]|\\.)*"', source):
        if re.search(r'[\u4e00-\u9fff]', match[0]):
            try:
                keys.add(json.loads(match[0]))
            except ValueError:
                pass
keys = sorted(keys)
catalog = {key: [key] * len(LOCALES) for key in keys}
target = ROOT / 'assets/locales/ui.json'
target.parent.mkdir(exist_ok=True)
checkpoint = ROOT / 'test-data/ui-translation-drafts.json'
checkpoint.parent.mkdir(exist_ok=True)
completed = json.loads(checkpoint.read_text(encoding='utf-8')) if checkpoint.exists() else {}
if target.exists():
    for key, values in json.loads(target.read_text(encoding='utf-8')).items():
        for locale, value in zip(LOCALES[1:], values[1:]):
            completed.setdefault(locale, {})[key] = value
placeholder = re.compile(r'\{[^{}\r\n]*\}')

def translate(job):
    locale, subset = job
    originals = {}
    blocks = []
    for index, key in subset:
        replacements = []
        def protect(match):
            token = f'__PH{len(replacements):03}__'
            replacements.append((token, match[0]))
            return token
        value = placeholder.sub(protect, key)
        originals[index] = replacements
        blocks.append(f'__XC{index:04}__\n{value}')
    query = '\n'.join(blocks)
    url = 'https://translate.googleapis.com/translate_a/single?' + urllib.parse.urlencode(
        dict(client='gtx', sl='zh-CN', tl=locale, dt='t', q=query))
    with urllib.request.urlopen(url, timeout=40) as response:
        result = json.load(response)
    text = ''.join(part[0] for part in result[0] if part[0])
    pieces = re.split(r'__XC(\d{4})__', text)
    translated = {}
    for offset in range(1, len(pieces), 2):
        index, value = int(pieces[offset]), pieces[offset + 1].strip()
        for token, original in originals[index]:
            if token not in value:
                raise ValueError(f'missing placeholder: {locale} {index}')
            value = value.replace(token, original)
        if not value or placeholder.findall(value) != placeholder.findall(keys[index]):
            raise ValueError(f'invalid translation: {locale} {index}')
        translated[index] = value
    if set(translated) != set(originals):
        raise ValueError(f'missing keys: {locale}')
    return locale, translated

chunks, current, size = [], [], 0
for index, key in enumerate(keys):
    cost = len(urllib.parse.quote(key)) + 45
    if current and size + cost > 5500:
        chunks.append(current)
        current, size = [], 0
    current.append((index, key))
    size += cost
if current:
    chunks.append(current)
jobs = []
for locale in LOCALES[1:]:
    for chunk in chunks:
        missing = [(i, key) for i, key in chunk if key not in completed.get(locale, {})]
        if missing:
            jobs.append((locale, missing))
with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
    for future in concurrent.futures.as_completed([executor.submit(translate, job) for job in jobs]):
        locale, result = future.result()
        for index, text in result.items():
            completed.setdefault(locale, {})[keys[index]] = text
        checkpoint.write_text(json.dumps(completed, ensure_ascii=False), encoding='utf-8')
        print(f'translated {locale}: {len(result)} keys', flush=True)

for locale in LOCALES[1:]:
    for key in keys:
        catalog[key][LOCALES.index(locale)] = completed[locale][key]
target = ROOT / 'assets/locales/ui.json'
target.parent.mkdir(exist_ok=True)
target.write_text(json.dumps(catalog, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
print(f'Wrote {len(catalog)} UI keys / {len(LOCALES)} languages', flush=True)
