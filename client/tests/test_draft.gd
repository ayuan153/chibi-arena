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

func test_draft_available_round1():
	setup_shop()
	var choices = gm.get_draft_choices(0)
	return assert_eq(choices.size(), 3, "draft choices round 1")

func test_draft_available_round3():
	setup_shop()
	# Draft round 1, pick a hero
	gm.apply_player_action(0, "DraftHero", "0")
	gm.apply_player_action(1, "DraftHero", "0")
	advance_round()  # round 2 (not draft)
	advance_round()  # round 3 (draft)
	var choices = gm.get_draft_choices(0)
	return assert_eq(choices.size(), 3, "draft choices round 3")

func test_draft_hero_adds_to_roster():
	setup_shop()
	var before = gm.get_heroes(0).size()
	gm.apply_player_action(0, "DraftHero", "0")
	var after = gm.get_heroes(0).size()
	if after <= before:
		return "hero count didn't increase: before=%d after=%d" % [before, after]
	return true

func test_hero_reroll_costs_2g():
	setup_shop()
	# First draft a hero so we have one to reroll
	gm.apply_player_action(0, "DraftHero", "0")
	gm.apply_player_action(1, "DraftHero", "0")
	var gold_before = gm.get_gold(0)
	gm.apply_player_action(0, "RerollHero", "0")
	var gold_after = gm.get_gold(0)
	return assert_eq(gold_before - gold_after, 2, "reroll cost")

func test_hero_reroll_generates_choices():
	setup_shop()
	gm.apply_player_action(0, "DraftHero", "0")
	gm.apply_player_action(1, "DraftHero", "0")
	gm.apply_player_action(0, "RerollHero", "0")
	var choices = gm.get_draft_choices(0)
	return assert_eq(choices.size(), 3, "reroll choices")

func test_hero_reroll_keeps_abilities():
	setup_shop()
	gm.apply_player_action(0, "DraftHero", "0")
	gm.apply_player_action(1, "DraftHero", "0")
	var hero_name = gm.get_heroes(0)[0]
	# Buy and equip an ability
	gm.set_gold(0, 10)
	var offerings = gm.get_shop_offerings(0)
	if offerings.size() > 0 and offerings[0] != "":
		gm.apply_player_action(0, "Buy", "0")
		var bench = gm.get_bench(0)
		if bench.size() > 0:
			gm.apply_player_action(0, "Equip", bench[0] + "," + hero_name)
			var equipped_before = gm.get_equipped_abilities(0, hero_name)
			# Reroll the hero
			gm.apply_player_action(0, "RerollHero", "0")
			gm.apply_player_action(0, "DraftHero", "0")
			var new_hero = gm.get_heroes(0)[0]
			var equipped_after = gm.get_equipped_abilities(0, new_hero)
			return assert_eq(equipped_after.size(), equipped_before.size(), "abilities preserved")
	return true
