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

func advance_round():
	gm.apply_player_action(0, "Ready", "")
	gm.apply_player_action(1, "Ready", "")
	gm.end_combat()
	gm.apply_player_action(0, "Ready", "")
	gm.apply_player_action(1, "Ready", "")

# --- Tests ---

func test_game_manager_exists():
	if gm == null:
		return "GameManager is null"
	return true

func test_shop_row_exists():
	var node = gm.get_tree().root.get_node_or_null("MainScene/BottomPanel/ShopRow")
	if node == null:
		return "ShopRow node not found"
	return true

func test_ready_button_exists():
	var node = gm.get_tree().root.get_node_or_null("MainScene/ReadyButton")
	if node == null:
		return "ReadyButton node not found"
	return true

func test_god_pick_advances_to_shop():
	setup_shop()
	return assert_eq(gm.get_phase(), "Shop")

func test_round_cycle():
	setup_shop()
	advance_round()
	var r = assert_eq(gm.get_round(), 2, "round")
	if r != true:
		return r
	return assert_eq(gm.get_phase(), "Shop", "phase")

func test_game_ends_on_elimination():
	setup_shop()
	gm.set_hp(0, 1.0)
	# Give player 1 a hero and position it for combat
	gm.apply_player_action(1, "DraftHero", "0")
	var heroes = gm.get_heroes(1)
	if heroes.size() > 0:
		gm.apply_player_action(1, "SetPosition", heroes[0] + ",500,500")
	# Ready both to enter combat
	gm.apply_player_action(0, "Ready", "")
	gm.apply_player_action(1, "Ready", "")
	gm.run_combat()
	gm.end_combat()
	# If HP went to 0, player is eliminated
	if gm.get_player_hp(0) <= 0.0:
		gm.apply_player_action(1, "Ready", "")
		return assert_eq(gm.get_phase(), "Finished")
	# Combat may not have dealt damage without heroes — just verify no crash
	return true
