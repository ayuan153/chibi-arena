extends SceneTree

var passed := 0
var failed := 0
var errors := []
var frame_count := 0

func _initialize():
	var scene = load("res://main.tscn").instantiate()
	root.add_child(scene)

func _process(_delta):
	frame_count += 1
	if frame_count < 3:
		return  # Wait a couple frames for ready() to fire

	if frame_count == 3:
		run_all_tests()
		print("\n========================================")
		print("Results: %d passed, %d failed" % [passed, failed])
		for e in errors:
			print("  FAIL: ", e)
		print("========================================")
		quit(0 if failed == 0 else 1)

func run_all_tests():
	var gm = root.get_node("MainScene/GameManager")
	if gm == null:
		print("ERROR: GameManager not found!")
		failed += 1
		return

	var test_scripts := [
		"res://tests/test_game_flow.gd",
		"res://tests/test_shop_mechanics.gd",
		"res://tests/test_draft.gd",
		"res://tests/test_equip.gd",
		"res://tests/test_combat.gd",
	]

	for script_path in test_scripts:
		var script = load(script_path)
		if script == null:
			print("[SKIP] Could not load: ", script_path)
			continue
		var test_obj = script.new()
		test_obj.gm = gm
		run_tests(test_obj, script_path)

func run_tests(obj, script_path: String):
	var methods = obj.get_method_list()
	for m in methods:
		if m["name"].begins_with("test_"):
			obj.gm.init_game(42, 2, "../data")
			var test_name = script_path.get_file() + "::" + m["name"]
			var result = obj.call(m["name"])
			if result == null or result is bool and result == true:
				passed += 1
				print("  PASS: ", test_name)
			else:
				failed += 1
				var msg = test_name + " -> " + str(result)
				errors.append(msg)
				print("  FAIL: ", test_name, " -> ", result)
