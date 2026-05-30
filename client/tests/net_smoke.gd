# Networked smoke test — drives GameManager against a real aa2-server.
# Run: godot --headless --path client --script tests/net_smoke.gd
# Approach: GameManager.new() + manual tick in SceneTree script.
extends SceneTree

func _init() -> void:
	var root := get_root()
	var gm := GameManager.new()
	root.add_child(gm)
	gm.init_game(42, 2, "../data")
	gm.connect_to_server("ws://127.0.0.1:9001")

	var deadline := Time.get_ticks_msec() + 15000

	# HANDSHAKE: wait for lobby
	while gm.get_lobby_player_count() == 0:
		if Time.get_ticks_msec() > deadline:
			print("SMOKE FAIL: timeout waiting for lobby handshake")
			quit(1)
			return
		gm.tick(0.1)
		await create_timer(0.05).timeout

	print("  handshake OK (lobby_player_count=%d)" % gm.get_lobby_player_count())

	# START: request game start, wait for first snapshot
	gm.start_game()
	while gm.get_player_count() == 0:
		if Time.get_ticks_msec() > deadline:
			print("SMOKE FAIL: timeout waiting for first snapshot after start")
			quit(1)
			return
		gm.tick(0.1)
		await create_timer(0.05).timeout

	print("  start OK (player_count=%d, phase=%s)" % [gm.get_player_count(), gm.get_phase()])

	# PICK GOD + READY
	var gods := gm.get_available_gods()
	if gods.size() == 0:
		print("SMOKE FAIL: no gods available")
		quit(1)
		return
	var god_name: String = gods[0]["name"]
	var pid := gm.get_my_player_id()
	gm.apply_player_action(pid, "PickGod", god_name)
	gm.apply_player_action(pid, "Ready", "")

	# Wait for phase to advance to Shop
	while gm.get_phase() != "Shop":
		if Time.get_ticks_msec() > deadline:
			print("SMOKE FAIL: timeout waiting for Shop phase (stuck at %s)" % gm.get_phase())
			quit(1)
			return
		gm.tick(0.1)
		await create_timer(0.05).timeout

	# Final assertion
	var gold := gm.get_gold(pid)
	if gold <= 0:
		print("SMOKE FAIL: expected gold > 0 in Shop, got %d" % gold)
		quit(1)
		return

	print("  shop OK (gold=%d)" % gold)
	print("SMOKE PASS")
	quit(0)
