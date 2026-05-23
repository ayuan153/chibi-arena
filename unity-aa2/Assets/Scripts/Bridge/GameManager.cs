using System;
using UnityEngine;

/// <summary>
/// Manages the aa2-ffi game context lifecycle.
/// Creates game on Start, ticks each Update, destroys on OnDestroy.
/// </summary>
public class GameManager : MonoBehaviour
{
    [SerializeField] private int seed = 42;
    [SerializeField] private int numPlayers = 2;
    [SerializeField] private string dataPath = "";

    private IntPtr _ctx;
    private bool _initialized;

    void Start()
    {
        string resolvedDataPath = string.IsNullOrEmpty(dataPath)
            ? System.IO.Path.Combine(Application.streamingAssetsPath, "data")
            : dataPath;

        string config = JsonUtility.ToJson(new GameConfig
        {
            seed = seed,
            num_players = numPlayers,
            data_path = resolvedDataPath
        });

        _ctx = AA2Bridge.aa2_create_game(config);
        if (_ctx == IntPtr.Zero)
        {
            Debug.LogError("[AA2] Failed to create game context");
            return;
        }

        _initialized = true;
        Debug.Log("[AA2] Game created successfully");

        // Log initial player view
        string view = AA2Bridge.PtrToStringAndFree(AA2Bridge.aa2_get_player_view(_ctx, 0));
        Debug.Log($"[AA2] Player 0 view: {view}");
    }

    void Update()
    {
        if (!_initialized) return;

        string events = AA2Bridge.PtrToStringAndFree(AA2Bridge.aa2_tick(_ctx, Time.deltaTime));
        if (!string.IsNullOrEmpty(events) && events != "[]")
        {
            Debug.Log($"[AA2] Events: {events}");
        }
    }

    void OnDestroy()
    {
        if (_initialized && _ctx != IntPtr.Zero)
        {
            AA2Bridge.aa2_destroy_game(_ctx);
            _ctx = IntPtr.Zero;
            _initialized = false;
            Debug.Log("[AA2] Game destroyed");
        }
    }

    /// <summary>Submit a player action (called from UI).</summary>
    public string PlayerAction(byte playerId, string actionJson)
    {
        if (!_initialized) return "{\"error\": \"not initialized\"}";
        return AA2Bridge.PtrToStringAndFree(AA2Bridge.aa2_player_action(_ctx, playerId, actionJson));
    }

    /// <summary>Run combat for the current round.</summary>
    public string RunCombat()
    {
        if (!_initialized) return "{\"error\": \"not initialized\"}";
        return AA2Bridge.PtrToStringAndFree(AA2Bridge.aa2_run_combat(_ctx));
    }

    /// <summary>Get player view as JSON.</summary>
    public string GetPlayerView(byte playerId)
    {
        if (!_initialized) return "{\"error\": \"not initialized\"}";
        return AA2Bridge.PtrToStringAndFree(AA2Bridge.aa2_get_player_view(_ctx, playerId));
    }

    [Serializable]
    private struct GameConfig
    {
        public int seed;
        public int num_players;
        public string data_path;
    }
}
