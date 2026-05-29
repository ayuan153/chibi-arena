extends RefCounted

var gm

func assert_eq(a, b, msg := ""):
	if a == b:
		return true
	return "expected %s got %s %s" % [str(b), str(a), msg]

func setup_shop():
	gm.apply_player_action(0, "PickGod", "Archmage")
	gm.apply_player_action(1, "PickGod", "Archmage")
	gm.apply_player_action(0, "Ready", "")
	gm.apply_player_action(1, "Ready", "")

# --- Tests ---

func test_damage_summary_after_combat():
	setup_shop()
	gm.apply_player_action(0, "DraftHero", "0")
	gm.apply_player_action(1, "DraftHero", "0")
	var hero0 = gm.get_heroes(0)[0]
	var hero1 = gm.get_heroes(1)[0]
	gm.apply_player_action(0, "SetPosition", hero0 + ",500,500")
	gm.apply_player_action(1, "SetPosition", hero1 + ",500,500")
	gm.apply_player_action(0, "Ready", "")
	gm.apply_player_action(1, "Ready", "")
	gm.run_combat()
	var s = gm.get_damage_summary(0)
	if s.size() <= 0:
		return "expected damage summary entries, got 0"
	if not s[0].has("unit_id"):
		return "missing key unit_id"
	if not s[0].has("name"):
		return "missing key name"
	if not s[0].has("team"):
		return "missing key team"
	if not s[0].has("damage"):
		return "missing key damage"
	var total := 0
	for entry in s:
		total += entry["damage"]
	if total <= 0:
		return "expected total damage > 0, got %d" % total
	if s[0]["damage"] < s[s.size() - 1]["damage"]:
		return "expected sorted descending"
	return true

func test_damage_summary_invalid_matchup_is_empty():
	var s = gm.get_damage_summary(999)
	if s.size() != 0:
		return "expected empty array for invalid matchup, got size %d" % s.size()
	return true
