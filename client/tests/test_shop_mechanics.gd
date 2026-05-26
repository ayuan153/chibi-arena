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

func test_buy_deducts_gold():
	setup_shop()
	var gold_before = gm.get_gold(0)
	var offerings = gm.get_shop_offerings(0)
	if offerings.size() == 0:
		return "no offerings"
	gm.apply_player_action(0, "Buy", "0")
	var gold_after = gm.get_gold(0)
	if gold_after >= gold_before:
		return "gold not deducted: before=%d after=%d" % [gold_before, gold_after]
	return true

func test_buy_adds_to_bench():
	setup_shop()
	var offerings = gm.get_shop_offerings(0)
	if offerings.size() == 0:
		return "no offerings"
	gm.apply_player_action(0, "Buy", "0")
	return assert_eq(gm.get_bench(0).size(), 1, "bench size")

func test_buy_fails_when_broke():
	setup_shop()
	gm.set_gold(0, 0)
	var result = gm.apply_player_action(0, "Buy", "0")
	if result == "ok":
		return "buy should have failed with 0 gold"
	return assert_eq(gm.get_bench(0).size(), 0, "bench should be empty")

func test_buy_fails_bench_full():
	setup_shop()
	gm.set_gold(0, 99)
	# Buy 5 abilities across multiple rerolls to fill bench
	var bought := 0
	for _attempt in range(20):
		if bought >= 5:
			break
		var offerings = gm.get_shop_offerings(0)
		for i in range(offerings.size()):
			if bought >= 5:
				break
			if offerings[i] != "":
				var r = gm.apply_player_action(0, "Buy", str(i))
				if r == "ok":
					bought += 1
		if bought < 5:
			gm.apply_player_action(0, "RerollShop", "")
	if bought < 5:
		return "could not fill bench, only bought %d" % bought
	# Now try to buy a 6th (should fail)
	gm.apply_player_action(0, "RerollShop", "")
	var offerings = gm.get_shop_offerings(0)
	for i in range(offerings.size()):
		if offerings[i] != "":
			# Check it's not a duplicate (would level up)
			var bench = gm.get_bench(0)
			var is_dup = false
			for b in bench:
				if b == offerings[i]:
					is_dup = true
					break
			if not is_dup:
				var result = gm.apply_player_action(0, "Buy", str(i))
				if result == "ok":
					return "buy should have failed with full bench"
				return true
	return "could not find non-duplicate offering to test"

func test_buy_levels_up_bypasses_bench_cap():
	setup_shop()
	gm.set_gold(0, 99)
	# Buy first ability
	gm.apply_player_action(0, "Buy", "0")
	var first_ability = gm.get_bench(0)[0]
	# Fill remaining bench slots with different abilities
	var bought := 1
	for _attempt in range(20):
		if bought >= 5:
			break
		gm.apply_player_action(0, "RerollShop", "")
		var offerings = gm.get_shop_offerings(0)
		for i in range(offerings.size()):
			if bought >= 5:
				break
			if offerings[i] != "" and offerings[i] != first_ability:
				var bench = gm.get_bench(0)
				var is_dup = false
				for b in bench:
					if b == offerings[i]:
						is_dup = true
						break
				if not is_dup:
					var r = gm.apply_player_action(0, "Buy", str(i))
					if r == "ok":
						bought += 1
	if bought < 5:
		return "could not fill bench to 5"
	# Now find the first_ability in shop (reroll until we see it)
	for _attempt in range(50):
		gm.apply_player_action(0, "RerollShop", "")
		var offerings = gm.get_shop_offerings(0)
		for i in range(offerings.size()):
			if offerings[i] == first_ability:
				var result = gm.apply_player_action(0, "Buy", str(i))
				if result != "ok":
					return "level-up should bypass bench cap, got: " + result
				return assert_eq(gm.get_ability_level(0, first_ability), 2, "level")
	return "could not find duplicate in shop after 50 rerolls"

func test_reroll_changes_offerings():
	setup_shop()
	var before = gm.get_shop_offerings(0)
	var gold_before = gm.get_gold(0)
	gm.apply_player_action(0, "RerollShop", "")
	var after = gm.get_shop_offerings(0)
	var gold_after = gm.get_gold(0)
	if gold_after >= gold_before:
		return "reroll didn't cost gold"
	if before == after:
		return "offerings unchanged after reroll"
	return true

func test_lock_preserves_offerings():
	setup_shop()
	var before = gm.get_shop_offerings(0)
	gm.apply_player_action(0, "LockShop", "")
	# Reroll should not change offerings when locked
	gm.apply_player_action(0, "RerollShop", "")
	var after = gm.get_shop_offerings(0)
	return assert_eq(after, before, "locked offerings should persist through reroll")

func test_upgrade_cost_round1():
	setup_shop()
	return assert_eq(gm.get_upgrade_cost(0), 10, "upgrade cost round 1")

func test_upgrade_cost_round3():
	setup_shop()
	advance_round()  # round 2
	advance_round()  # round 3
	# Decay: 2 rounds at level 1 = cost 10 - 2 = 8
	return assert_eq(gm.get_upgrade_cost(0), 8, "upgrade cost round 3")

func test_upgrade_increases_size():
	setup_shop()
	gm.set_gold(0, 20)
	var before_size = gm.get_shop_offerings(0).size()
	gm.apply_player_action(0, "UpgradeShop", "")
	var after_size = gm.get_shop_offerings(0).size()
	if after_size <= before_size:
		return "shop size didn't increase: before=%d after=%d" % [before_size, after_size]
	return true

func test_upgrade_rerolls_shop():
	setup_shop()
	gm.set_gold(0, 20)
	var before = gm.get_shop_offerings(0)
	gm.apply_player_action(0, "UpgradeShop", "")
	var after = gm.get_shop_offerings(0)
	# Size changed so they can't be equal, but verify content differs
	if before == after:
		return "offerings unchanged after upgrade"
	return true
