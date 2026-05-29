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

func test_sell_bench_ability_refunds_gold():
	setup_shop()
	gm.set_gold(0, 100)
	gm.apply_player_action(0, "Buy", "0")
	var bench = gm.get_bench(0)
	if bench.size() == 0:
		return "no ability on bench after buy"
	var name = bench[0]
	var gold_before = gm.get_gold(0)
	var level = gm.get_ability_level(0, name)
	gm.apply_player_action(0, "Sell", name)
	var gold_after = gm.get_gold(0)
	var r = assert_eq(gold_after, gold_before + 2 * level, "gold refund")
	if r != true:
		return r
	var bench_after = gm.get_bench(0)
	var r2 = assert_eq(bench_after.size(), bench.size() - 1, "bench shrunk by one")
	if r2 != true:
		return r2
	for ability in bench_after:
		if ability == name:
			return "ability still on bench after sell"
	return true

func test_sell_nonexistent_is_noop_or_safe():
	setup_shop()
	gm.set_gold(0, 50)
	var gold_before = gm.get_gold(0)
	gm.apply_player_action(0, "Sell", "NonexistentAbility999")
	var gold_after = gm.get_gold(0)
	return assert_eq(gold_after, gold_before, "gold unchanged after selling nonexistent")
