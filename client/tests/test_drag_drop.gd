# Tests for drag-and-drop glue methods.
# These call the NEW #[func] glue methods directly on the LoadoutUi/BoardUI nodes
# (reachable from the GameManager node via absolute paths) — the same methods that the
# get_drag_data/can_drop_data/drop_data virtuals delegate to. This exercises the real
# drag-drop payload + drop-handling code, not just the underlying apply_player_action.
extends RefCounted

var gm

const LOADOUT_PATH := "/root/MainScene/BottomPanel/LoadoutGrid"
const BOARD_PATH := "/root/MainScene/ArenaRegion/BoardUI"

func assert_eq(a, b, msg := ""):
	if a == b:
		return true
	return "expected %s got %s %s" % [str(b), str(a), msg]

func loadout():
	return gm.get_node(LOADOUT_PATH)

func board():
	return gm.get_node(BOARD_PATH)

func setup_shop():
	gm.apply_player_action(0, "PickGod", "Archmage")
	gm.apply_player_action(1, "PickGod", "Archmage")
	gm.apply_player_action(0, "Ready", "")
	gm.apply_player_action(1, "Ready", "")

func buy_one_to_bench():
	gm.set_gold(0, 10)
	gm.apply_player_action(0, "Buy", "0")

# --- Tests ---

func test_make_bench_payload_format():
	var lo = loadout()
	if lo == null:
		return "LoadoutUi node not found at " + LOADOUT_PATH
	setup_shop()
	buy_one_to_bench()
	var ability = gm.get_bench(0)[0]
	var payload = lo.make_bench_payload(0)
	var r = assert_eq(payload.get("kind", ""), "ability", "payload kind")
	if r != true:
		return r
	r = assert_eq(payload.get("src", ""), "bench", "payload src")
	if r != true:
		return r
	r = assert_eq(payload.get("ability", ""), ability, "payload ability name")
	if r != true:
		return r
	# Out-of-range index yields a payload with no ability (empty/guarded).
	var empty = lo.make_bench_payload(99)
	return assert_eq(empty.has("ability"), false, "out-of-range bench payload is empty")

func test_drop_equip_equips():
	var lo = loadout()
	if lo == null:
		return "LoadoutUi node not found"
	setup_shop()
	gm.apply_player_action(0, "DraftHero", "0")
	gm.apply_player_action(1, "DraftHero", "0")
	var hero_name = gm.get_heroes(0)[0]
	buy_one_to_bench()
	var ability = gm.get_bench(0)[0]
	var ok = lo.drop_equip(ability, 0)
	var r = assert_eq(ok, true, "drop_equip returns true")
	if r != true:
		return r
	var equipped = gm.get_equipped_abilities(0, hero_name)
	r = assert_eq(equipped.size(), 1, "one ability equipped via drop_equip")
	if r != true:
		return r
	return assert_eq(equipped[0], ability, "correct ability equipped")

func test_drop_sell_refunds():
	var lo = loadout()
	if lo == null:
		return "LoadoutUi node not found"
	setup_shop()
	buy_one_to_bench()
	var ability = gm.get_bench(0)[0]
	var gold_before = gm.get_gold(0)
	var level = gm.get_ability_level(0, ability)
	var ok = lo.drop_sell(ability)
	var r = assert_eq(ok, true, "drop_sell returns true")
	if r != true:
		return r
	r = assert_eq(gm.get_gold(0), gold_before + 2 * level, "gold refunded via drop_sell")
	if r != true:
		return r
	return assert_eq(gm.get_bench(0).size(), 0, "bench empty after drop_sell")

func test_drop_unequip_returns_to_bench():
	var lo = loadout()
	if lo == null:
		return "LoadoutUi node not found"
	setup_shop()
	gm.apply_player_action(0, "DraftHero", "0")
	gm.apply_player_action(1, "DraftHero", "0")
	var hero_name = gm.get_heroes(0)[0]
	buy_one_to_bench()
	var ability = gm.get_bench(0)[0]
	lo.drop_equip(ability, 0)
	var ok = lo.drop_unequip(ability, hero_name)
	var r = assert_eq(ok, true, "drop_unequip returns true")
	if r != true:
		return r
	r = assert_eq(gm.get_equipped_abilities(0, hero_name).size(), 0, "unequipped via drop")
	if r != true:
		return r
	return assert_eq(gm.get_bench(0).size(), 1, "ability back on bench")

func test_make_hero_payload_and_reposition():
	var bd = board()
	if bd == null:
		return "BoardUI node not found at " + BOARD_PATH
	setup_shop()
	gm.apply_player_action(0, "DraftHero", "0")
	gm.apply_player_action(1, "DraftHero", "0")
	var hero_name = gm.get_heroes(0)[0]
	var payload = bd.make_hero_payload(0)
	var r = assert_eq(payload.get("kind", ""), "hero", "hero payload kind")
	if r != true:
		return r
	r = assert_eq(payload.get("hero", ""), hero_name, "hero payload name")
	if r != true:
		return r
	gm.apply_player_action(0, "SetPosition", hero_name + ",500,1500")
	var pos_before = gm.get_hero_position(0, hero_name)
	var ok = bd.reposition_hero(hero_name, 1200.0, 1800.0)
	r = assert_eq(ok, true, "reposition_hero returns true")
	if r != true:
		return r
	return assert_eq(gm.get_hero_position(0, hero_name) != pos_before, true, "position changed via reposition_hero")
