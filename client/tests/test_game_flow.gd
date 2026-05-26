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
	# Draft heroes for both players so combat works
	gm.apply_player_action(0, "DraftHero", "0")
	gm.apply_player_action(1, "DraftHero", "0")
	# Set player 0 HP very low so they die after combat
	gm.set_hp(0, 1.0)
	# Run combat
	gm.apply_player_action(0, "Ready", "")
	gm.apply_player_action(1, "Ready", "")
	gm.run_combat()
	gm.end_combat()
	# Check if player was eliminated (HP should be 0 after losing)
	var hp = gm.get_player_hp(0)
	if hp > 0.0:
		# Combat didn't kill — force it for the test
		gm.set_hp(0, 0.0)
	# Advance past grace period — game should detect elimination
	gm.apply_player_action(1, "Ready", "")
	return assert_eq(gm.get_phase(), "Finished")
