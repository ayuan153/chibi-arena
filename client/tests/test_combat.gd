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

func test_combat_produces_events():
	setup_shop()
	# Draft heroes for both players
	gm.apply_player_action(0, "DraftHero", "0")
	gm.apply_player_action(1, "DraftHero", "0")
	var hero0 = gm.get_heroes(0)[0]
	var hero1 = gm.get_heroes(1)[0]
	# Position them
	gm.apply_player_action(0, "SetPosition", hero0 + ",500,500")
	gm.apply_player_action(1, "SetPosition", hero1 + ",500,500")
	# Enter combat
	gm.apply_player_action(0, "Ready", "")
	gm.apply_player_action(1, "Ready", "")
	gm.run_combat()
	var events = gm.get_combat_event_count(0)
	if events <= 0:
		return "no combat events produced"
	return true

func test_loser_takes_damage():
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
	gm.end_combat()
	var hp0 = gm.get_player_hp(0)
	var hp1 = gm.get_player_hp(1)
	if hp0 >= 200.0 and hp1 >= 200.0:
		return "no player took damage: hp0=%f hp1=%f" % [hp0, hp1]
	return true

func test_combat_no_crash_without_heroes():
	setup_shop()
	# No heroes drafted, just enter combat
	gm.apply_player_action(0, "Ready", "")
	gm.apply_player_action(1, "Ready", "")
	var result = gm.run_combat()
	# Should not crash, result may be false (no matchups)
	return true
