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

# Finished phase test
func test_game_ends_on_elimination():
	setup_shop()
	gm.apply_player_action(0, "DraftHero", "0")
	gm.apply_player_action(1, "DraftHero", "0")
	gm.set_hp(0, 1.0)
	gm.apply_player_action(0, "Ready", "")
	gm.apply_player_action(1, "Ready", "")
	gm.run_combat()
	gm.end_combat()
	return assert_eq(gm.get_phase(), "Finished")

# --- Sprint 3 UI Tests ---

func test_god_pick_grid_has_buttons():
	var grid = gm.get_tree().root.get_node_or_null("MainScene/GodPickUI/RootVBox/Content/GodGrid")
	if grid == null:
		return "GodGrid node not found"
	if grid.get_child_count() == 0:
		return "GodGrid has no buttons"
	return true

func test_god_pick_confirm_advances_phase():
	gm.apply_player_action(0, "PickGod", "Archmage")
	gm.apply_player_action(1, "PickGod", "Archmage")
	gm.apply_player_action(0, "Ready", "")
	gm.apply_player_action(1, "Ready", "")
	return assert_eq(gm.get_phase(), "Shop", "phase after god pick")

func test_summary_toggle():
	var main = gm.get_tree().root.get_node_or_null("MainScene")
	var sb = gm.get_tree().root.get_node_or_null("MainScene/ScoreboardUI")
	if sb == null:
		return "ScoreboardUI node not found"
	if sb.visible:
		return "ScoreboardUI should start hidden"
	main.toggle_summary()
	if not sb.visible:
		return "ScoreboardUI should be visible after first toggle"
	main.toggle_summary()
	if sb.visible:
		return "ScoreboardUI should be hidden after second toggle"
	return true

func test_scoreboard_hidden_during_god_pick():
	var sb = gm.get_tree().root.get_node_or_null("MainScene/ScoreboardUI")
	if sb == null:
		return "ScoreboardUI node not found"
	return assert_eq(sb.visible, false, "ScoreboardUI should be hidden during GodPick")
