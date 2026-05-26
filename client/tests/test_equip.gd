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

func test_equip_from_bench():
	setup_shop()
	# Draft a hero
	gm.apply_player_action(0, "DraftHero", "0")
	gm.apply_player_action(1, "DraftHero", "0")
	var hero_name = gm.get_heroes(0)[0]
	# Buy an ability
	gm.set_gold(0, 10)
	gm.apply_player_action(0, "Buy", "0")
	var bench_before = gm.get_bench(0).size()
	var ability = gm.get_bench(0)[0]
	# Equip it
	gm.apply_player_action(0, "Equip", ability + "," + hero_name)
	var bench_after = gm.get_bench(0).size()
	var equipped = gm.get_equipped_abilities(0, hero_name).size()
	var r = assert_eq(bench_after, bench_before - 1, "bench shrinks")
	if r != true:
		return r
	return assert_eq(equipped, 1, "equipped grows")

func test_unequip_to_bench():
	setup_shop()
	gm.apply_player_action(0, "DraftHero", "0")
	gm.apply_player_action(1, "DraftHero", "0")
	var hero_name = gm.get_heroes(0)[0]
	gm.set_gold(0, 10)
	gm.apply_player_action(0, "Buy", "0")
	var ability = gm.get_bench(0)[0]
	gm.apply_player_action(0, "Equip", ability + "," + hero_name)
	# Now unequip
	var bench_before = gm.get_bench(0).size()
	gm.apply_player_action(0, "Unequip", ability + "," + hero_name)
	var bench_after = gm.get_bench(0).size()
	var equipped = gm.get_equipped_abilities(0, hero_name).size()
	var r = assert_eq(bench_after, bench_before + 1, "bench grows")
	if r != true:
		return r
	return assert_eq(equipped, 0, "equipped shrinks")

func test_swap_abilities():
	setup_shop()
	gm.apply_player_action(0, "DraftHero", "0")
	gm.apply_player_action(1, "DraftHero", "0")
	var hero_name = gm.get_heroes(0)[0]
	gm.set_gold(0, 99)
	# Buy and equip 2 different abilities
	gm.apply_player_action(0, "Buy", "0")
	var a1 = gm.get_bench(0)[0]
	gm.apply_player_action(0, "Equip", a1 + "," + hero_name)
	gm.apply_player_action(0, "RerollShop", "")
	gm.apply_player_action(0, "Buy", "0")
	var bench = gm.get_bench(0)
	if bench.size() == 0:
		return "no second ability on bench"
	var a2 = bench[0]
	gm.apply_player_action(0, "Equip", a2 + "," + hero_name)
	# Verify order
	var before = gm.get_equipped_abilities(0, hero_name)
	if before.size() < 2:
		return "need 2 equipped abilities"
	# Swap
	gm.apply_player_action(0, "SwapAbilities", hero_name + ",0,1")
	var after = gm.get_equipped_abilities(0, hero_name)
	var r = assert_eq(after[0], before[1], "slot 0 swapped")
	if r != true:
		return r
	return assert_eq(after[1], before[0], "slot 1 swapped")

func test_level_up_on_duplicate():
	setup_shop()
	gm.set_gold(0, 99)
	# Buy first ability
	gm.apply_player_action(0, "Buy", "0")
	var ability = gm.get_bench(0)[0]
	var bench_after_first = gm.get_bench(0).size()
	# Find same ability in shop via rerolls
	for _attempt in range(50):
		gm.apply_player_action(0, "RerollShop", "")
		var offerings = gm.get_shop_offerings(0)
		for i in range(offerings.size()):
			if offerings[i] == ability:
				gm.apply_player_action(0, "Buy", str(i))
				var level = gm.get_ability_level(0, ability)
				var r = assert_eq(level, 2, "level up")
				if r != true:
					return r
				return assert_eq(gm.get_bench(0).size(), bench_after_first, "bench unchanged")
	return "could not find duplicate in 50 rerolls"
