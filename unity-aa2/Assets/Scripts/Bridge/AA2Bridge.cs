using System;
using System.Runtime.InteropServices;

/// <summary>
/// P/Invoke bindings for the aa2-ffi native Rust library.
/// All complex data crosses the boundary as JSON strings.
/// </summary>
public static class AA2Bridge
{
    private const string LibName = "aa2_ffi";

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr aa2_create_game(string configJson);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    public static extern void aa2_destroy_game(IntPtr ctx);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr aa2_tick(IntPtr ctx, float dt);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr aa2_player_action(IntPtr ctx, byte playerId, string actionJson);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr aa2_run_combat(IntPtr ctx);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr aa2_get_player_view(IntPtr ctx, byte playerId);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr aa2_get_combat_replay(IntPtr ctx, byte matchupIndex);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl)]
    public static extern void aa2_free_string(IntPtr str);

    /// <summary>
    /// Helper: Marshal a returned string pointer to a C# string, then free the native memory.
    /// </summary>
    public static string PtrToStringAndFree(IntPtr ptr)
    {
        if (ptr == IntPtr.Zero) return null;
        string result = Marshal.PtrToStringAnsi(ptr);
        aa2_free_string(ptr);
        return result;
    }
}
