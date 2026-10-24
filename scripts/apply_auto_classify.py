import sqlite3
import json
import sys

sys.stdout.reconfigure(encoding='utf-8')

conn = sqlite3.connect('D:/Programming/Personal_projects_D/TikTok_Scan/data/leadfinder.db')
cursor = conn.cursor()

def has_cyrillic(s: str) -> bool:
    return any('\u0400' <= c <= '\u04FF' for c in s)

def has_latin(s: str) -> bool:
    return any(('a' <= c <= 'z') or ('A' <= c <= 'Z') for c in s)

def is_noise(name: str) -> bool:
    low = name.strip().lower()
    if not low:
        return True
    static_noise = {
        "no time", "time to", "to die",
        "ai vie", "ly ма", "my wc", "ui ат", "да fay", "жо fy",
        "запазен запазен", "класа кове", "павликеки запазен", "захарсът дигнах",
        "дигнах сътсемно", "анатем мосвамви", "гедебог тукбургаз", "радъв бългурското",
        "хора", "охранител", "водеща", "пиян полицай", "инфлация икономика",
        "икономика славивасилев", "желад", "софия вар", "стоимен врагове",
    }
    if low in static_noise:
        return True
    words = low.split()
    if not words or len(words) > 4:
        return True
    has_any_cyr = any(has_cyrillic(w) for w in words)
    has_any_lat = any(has_latin(w) for w in words)
    if has_any_cyr and has_any_lat:
        return True
    if len(words) == 2 and any(len(w) < 3 for w in words):
        return True
    return False

def detect_type(name: str, role: str = "") -> str:
    low = name.strip().lower()
    r_low = (role or "").strip().lower()

    has_person_title = any(low.startswith(p) for p in [
        "инж. ", "инженер ", "д-р ", "доктор ", "проф. ", "професор ",
        "доц. ", "доцент ", "адв. ", "адвокат "
    ])

    is_human_role = any(k in r_low for k in [
        "кмет", "министър", "съдия", "прокурор", "депутат", "председател",
        "директор", "политик", "член", "лекар", "журналист", "водещ",
        "автор", "създател"
    ])

    if has_person_title:
        return "person"

    group_keys = [
        "партия", "коалиция", "движение", "съюз", "асоциация", "синдикат", "фондация",
        "герб", "възраждане", "бсп", "дпс", "итн", "пп-дб", "демократична българия",
        "продължаваме промяната", "прогресивна българия", "величие", "мечове", "църква",
        "секта", "организация"
    ]
    if any(k in low for k in group_keys) or (not is_human_role and any(k in r_low for k in ["партия", "движение", "коалиция"])):
        return "group"

    inst_keys = [
        "община", "съд", "прокуратура", "следствие", "комисия", "европейска комисия",
        "министерство", "съвет", "агенция", "парламент", "събрание", "народно събрание",
        "инспекторат", "институт", "комитет", "фонд", "бюро", "дирекция", "управление",
        "европейски съюз", "nato", "нато", "оон", "юнеско", "сзо", "мвр", "данс", "гдбоп",
        "нап", "цик", "бнб", "кзк", "крс", "пентагон", "кремъл", "бял дом", "болница",
        "бта", "български лев", "еврозона", "президентство", "кметство", "университет", "бан"
    ]
    if any(k in low for k in inst_keys) or (not is_human_role and any(k in r_low for k in ["институция", "орган", "комисия", "съд", "община", "болница"])):
        return "institution"

    words = low.split()
    has_tv_token = any(w in ["тв", "tv"] for w in words)

    chan_keys = [
        "the clashers", "clashers", "новини софия", "балкан президент",
        "канал", "телевизия", "podcast", "подкаст", "шоу", "медия", "новини",
        "студио", "studio", "youtube", "тикток", "tiktok", "бтв", "btv",
        "нова тв", "нова телевизия", "бнт", "bnt", "евронюз", "свободна европа", "дойче веле", "цанов"
    ]
    if low.endswith(".bg") or ".bg " in low or any(k in low for k in chan_keys) or has_tv_token or (not is_human_role and any(k in r_low for k in ["канал", "медия", "предаване"])):
        return "channel"

    country_keys = [
        "българия", "русия", "сащ", "украйна", "германия", "франция", "великобритания",
        "китай", "турция", "гърция", "гръцко", "румъния", "сърбия", "северна македония",
        "македония", "израел", "палестина", "иран", "ирак", "сирия", "полша", "италия",
        "испания", "словения", "словакия", "унгария", "австрия", "швейцария", "европа",
        "холандия", "нидерландия", "белгия", "чехия", "дания", "швеция", "норвегия", "финландия"
    ]
    if low.startswith("република ") or low.startswith("кралство ") or low.startswith("държава") or any(k in low for k in country_keys) or (not is_human_role and any(k in r_low for k in ["държава", "страна"])):
        return "country"

    return "person"

rows = cursor.execute("SELECT id, canonical_name, role, tags_json, category, entity_type FROM people").fetchall()

pruned = 0
updated = 0

for pid, name, role, tags_json, category, entity_type in rows:
    if is_noise(name):
        cursor.execute("DELETE FROM video_people WHERE person_id = ?", (pid,))
        cursor.execute("DELETE FROM people WHERE id = ?", (pid,))
        pruned += 1
        print(f"[PRUNED] {name}")
        continue

    detected = detect_type(name, role)
    tags = []
    if tags_json:
        try:
            tags = json.loads(tags_json)
        except Exception:
            tags = []

    tag_to_add = None
    cat_to_set = None

    if detected == "institution":
        tag_to_add = "институция"
        cat_to_set = "институция"
    elif detected == "channel":
        tag_to_add = "канал"
        cat_to_set = "медия"
    elif detected == "country":
        tag_to_add = "държава"
        cat_to_set = "държава"
    elif detected == "group":
        tag_to_add = "политическа партия"
        cat_to_set = "политика"
    elif detected == "person":
        tags = [t for t in tags if t not in ["институция", "канал", "държава", "политическа партия"]]
        if category in ["институция", "медия", "държава"]:
            cat_to_set = None

    if tag_to_add and not any(t.lower() == tag_to_add.lower() for t in tags):
        tags.append(tag_to_add)

    new_tags_json = json.dumps(tags, ensure_ascii=False)
    cursor.execute(
        "UPDATE people SET entity_type = ?, category = ?, tags_json = ? WHERE id = ?",
        (detected, cat_to_set if cat_to_set is not None else category, new_tags_json, pid)
    )
    updated += 1
    print(f"[CLASSIFIED] {name} => {detected} (cat: {cat_to_set if cat_to_set is not None else category}, tags: {tags})")

conn.commit()
conn.close()

print(f"\nDone: {updated} updated, {pruned} pruned.")
