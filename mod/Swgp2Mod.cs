// SWGP2 Mod - keyboard cheats for Super Woden GP 2.
//
// No menu at all. Just:
//   F7  - repair the car instantly
//   F8  - increase acceleration multiplier (step +0.5)
//   F9  - decrease acceleration multiplier (step -0.5)
//   F6  - disable race filters (no CV/year/tag/dealership limits in any race)
//
// The multiplier is applied every frame to MainCar.CarPower (the car's
// acceleration). A small label at the screen center shows the current
// multiplier, or "car repaired" / "race filters disabled" briefly.
//
// Race filters are disabled by clearing the restriction fields on the loaded
// event data objects (LeagueData / WeekendData / RallyData): MaxCV, MinYear,
// MaxYear, RestrictTags, RequiredTags, RequiredBrand, ExclusiveAllowedCars.
// This is a one-way toggle for the whole game session; restart the game to
// get the filters back.
//
// Access to game types goes through reflection so the mod survives game
// patches that only rename/shuffle things. Missing fields/methods are
// tolerated - the hotkey for a missing field just does nothing.
//
// Build (see mod\build.ps1):
//   csc.exe /target:library /noconfig Swgp2Mod.cs /r:<Managed>\mscorlib.dll ^
//     /r:UnityEngine.CoreModule.dll /r:UnityEngine.IMGUIModule.dll ^
//     /r:UnityEngine.InputLegacyModule.dll /r:UnityEngine.TextRenderingModule.dll ^
//     /r:Assembly-CSharp.dll /out:Swgp2Mod.dll

using System;
using System.Collections.Generic;
using System.IO;
using System.Reflection;
using UnityEngine;

namespace Swgp2Mod
{
    public static class Entry
    {
        private static bool _loaded;

        // Fallback for the Managed-folder delivery (harmless when using the
        // injector; guarded against double-loading).
        [UnityEngine.RuntimeInitializeOnLoadMethod(UnityEngine.RuntimeInitializeLoadType.AfterSceneLoad)]
        public static void AutoStart()
        {
            Main();
        }

        public static void Main()
        {
            if (_loaded) return;
            try
            {
                var host = new GameObject("Swgp2Mod_Host");
                UnityEngine.Object.DontDestroyOnLoad(host);
                host.AddComponent<ModBehaviour>();
                _loaded = true;
                Log("mod loaded (F7 repair, F8 +, F9 -)");
            }
            catch (Exception e)
            {
                Log("failed to load: " + e);
            }
        }

        internal static void Log(string msg)
        {
            try
            {
                var dir = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "swgp2-tool");
                Directory.CreateDirectory(dir);
                File.AppendAllText(Path.Combine(dir, "mod.log"),
                    DateTime.Now.ToString("u") + " swgp2-mod: " + msg + "\n");
            }
            catch { /* logging must never break the game */ }
        }
    }

    public class ModBehaviour : MonoBehaviour
    {
        private const float MinMult = 0.5f, MaxMult = 5.0f, Step = 0.5f;
        private const float MessageSeconds = 3.0f;
        private const float FadeSeconds = 0.75f;

        private float _mult = 1.0f;
        private string _toast;          // transient message shown instead of the multiplier
        private float _toastUntil;      // Time.time when the text fully disappears
        private bool _raceFiltersDisabled;
        private float _nextFilterSweep;
        private int _filtersLogCounter;

        private object _car;            // MainCar instance (cached)
        private long _powerSnapshot = long.MinValue;
        private ulong _lifeSnapshot = ulong.MaxValue;
        private bool _inputDisabledLogged;

        // ---------------------------------------------------------- lifecycle
        void Update()
        {
            try
            {
                RefreshCar();
                if (Pressed(KeyCode.F7)) DoRepair();
                if (Pressed(KeyCode.F8)) DoBoost(+Step);
                if (Pressed(KeyCode.F9)) DoBoost(-Step);
                if (Pressed(KeyCode.F6)) DisableRaceFilters(true);

                if (_car != null && _powerSnapshot != long.MinValue && _powerSnapshot != 0)
                    SetLong(_car, "CarPower", (long)(_powerSnapshot * _mult));

                if (Time.time >= _nextFilterSweep)
                {
                    _nextFilterSweep = Time.time + 0.5f;
                    SweepRaceFilters();
                }
            }
            catch (Exception e) { Entry.Log("update: " + e.Message); }
        }

        void DoRepair()
        {
            if (_car == null) return;
            if (_lifeSnapshot != ulong.MaxValue)
                SetField(_car, "Life", _lifeSnapshot);
            _toast = "car repaired";
            _toastUntil = Time.time + MessageSeconds;
        }

        void DoBoost(float delta)
        {
            float next = Math.Max(MinMult, Math.Min(MaxMult, _mult + delta));
            if (Math.Abs(next - _mult) < 0.001f) return;
            _mult = next;
            _toast = null; // label shows the new multiplier
            _toastUntil = Time.time + MessageSeconds;
        }

        // ---------------------------------------------------------- race filters
        const int CVLimitDisabled = 9999, YearMin = 1900, YearMax = 65535;

        void DisableRaceFilters(bool fromKey)
        {
            if (fromKey)
            {
                _raceFiltersDisabled = true;
                _toast = "race filters disabled";
                _toastUntil = Time.time + MessageSeconds;
            }
            // keep applying on a timer as well (scene reloads recreate event data)
            SweepRaceFilters();
        }

        static Type GetTypeAny(string name)
        {
            foreach (var asm in AppDomain.CurrentDomain.GetAssemblies())
            {
                var t = asm.GetType(name);
                if (t != null) return t;
            }
            return null;
        }

        static IEnumerable<UnityEngine.Object> FindAllOfType(Type t)
        {
            var objType = typeof(UnityEngine.Object);
            var m = objType.GetMethod("FindObjectsOfType", new[] { typeof(Type) });
            if (m == null) yield break;
            object all;
            try
            {
                all = m.Invoke(null, new object[] { t });
            }
            catch (Exception e)
            {
                Entry.Log("FindObjectsOfType(" + t.Name + ") failed: " + e.Message);
                yield break;
            }
            if (all is System.Collections.IEnumerable en)
                foreach (var o in en)
                    if (o is UnityEngine.Object uo) yield return uo;
        }

        void SweepRaceFilters()
        {
            if (!_raceFiltersDisabled)
            {
                // first press or auto on load: disable once
                _raceFiltersDisabled = true;
            }
            int touched = 0;
            foreach (var tname in new[] { "LeagueData", "WeekendData", "RallyData" })
            {
                var t = GetTypeAny(tname);
                if (t == null) continue;
                foreach (var o in FindAllOfType(t))
                {
                    SetField(o, "MaxCV", (ushort)CVLimitDisabled);
                    SetField(o, "MinYear", (ushort)YearMin);
                    SetField(o, "MaxYear", (ushort)YearMax);
                    SetField(o, "RestrictTags", string.Empty);
                    SetField(o, "RequiredTags", string.Empty);
                    SetField(o, "RequiredBrand", string.Empty);
                    var exclusive = GetField(o, "ExclusiveAllowedCars");
                    if (exclusive is System.Collections.IList list) list.Clear();
                    touched++;
                }
            }
            if (touched != _filtersLogCounter)
            {
                _filtersLogCounter = touched;
                if (touched > 0)
                {
                    Entry.Log($"race filters disabled on {touched} data object(s)");
                    if (_toast == null)
                    {
                        _toast = "race filters disabled";
                        _toastUntil = Time.time + MessageSeconds;
                    }
                }
            }
        }

        // ---------------------------------------------------------- car lookup
        void RefreshCar()
        {
            if (_car != null && ((UnityEngine.Object)_car) != null)
                return;
            _car = null;
            _powerSnapshot = long.MinValue;
            _lifeSnapshot = ulong.MaxValue;
            try
            {
                var objType = typeof(UnityEngine.Object);
                var carType = Type.GetType("MainCar, Assembly-CSharp");
                if (carType == null) { Entry.Log("MainCar type not found"); return; }

                var plural = objType.GetMethod("FindObjectsOfType", new[] { typeof(Type) });
                if (plural != null)
                {
                    var all = plural.Invoke(null, new object[] { carType }) as System.Collections.IEnumerable;
                    if (all != null)
                    {
                        object first = null;
                        foreach (var o in all)
                        {
                            if (first == null) first = o;
                            var rival = GetField(o, "Rival");
                            if (rival is bool r && !r) { _car = o; break; }
                        }
                        if (_car == null) _car = first; // no clear player car - take the first
                    }
                }
                if (_car == null)
                {
                    var single = objType.GetMethod("FindObjectOfType", new[] { typeof(Type) });
                    _car = single != null
                        ? single.Invoke(null, new object[] { carType })
                        : objType.GetMethod("FindObjectOfType", new Type[0])?.MakeGenericMethod(carType).Invoke(null, null);
                }

                if (_car != null)
                {
                    _powerSnapshot = GetLong(_car, "CarPower") ?? long.MinValue;
                    _lifeSnapshot = GetULong(_car, "Life") ?? ulong.MaxValue;
                    Entry.Log($"car #{GetField(_car, "CarId")} attached (power {_powerSnapshot}, life {_lifeSnapshot})");
                }
            }
            catch (Exception e)
            {
                Entry.Log("car lookup failed: " + e.Message);
            }
        }

        // ---------------------------------------------------------- overlay
        void OnGUI()
        {
            try
            {
                float remain = _toastUntil - Time.time;
                if (remain <= 0f) return; // nothing to show right now
                float alpha = Mathf.Clamp01(remain / FadeSeconds); // fade-out in the last 0.75s

                string text = _toast != null
                    ? _toast
                    : (_mult <= 1.0f ? "1.0 (default)" : _mult.ToString("0.0"));

                var style = new GUIStyle(GUI.skin.label)
                {
                    alignment = TextAnchor.MiddleCenter,
                    fontSize = 30,
                    normal = { textColor = new Color(1f, 1f, 1f, alpha) }
                };
                var rect = new Rect(Screen.width / 2 - 200, Screen.height / 2 - 24, 400, 48);
                GUI.Label(rect, text, style);
            }
            catch (Exception e)
            {
                Entry.Log("gui: " + e.Message);
            }
        }

        // ----------------------------------------------------- input handling
        bool Pressed(KeyCode legacy)
        {
            try
            {
                return UnityEngine.Input.GetKeyDown(legacy);
            }
            catch (Exception e)
            {
                // legacy input disabled by the game (new Input System only) -
                // log once per session so we can tell from mod.log
                if (!_inputDisabledLogged)
                {
                    Entry.Log("legacy Input disabled: " + e.Message);
                    _inputDisabledLogged = true;
                }
                return false;
            }
        }

        // -------------------------------------------------- field access glue
        static FieldInfo FindField(object o, string name)
        {
            var t = o.GetType();
            while (t != null)
            {
                var f = t.GetField(name,
                    BindingFlags.Public | BindingFlags.Instance | BindingFlags.NonPublic | BindingFlags.FlattenHierarchy);
                if (f != null) return f;
                t = t.BaseType;
            }
            return null;
        }

        static object GetField(object o, string name)
        {
            var f = FindField(o, name);
            return f?.GetValue(o);
        }

        static long? GetLong(object o, string name)
        {
            var v = GetField(o, name);
            if (v == null) return null;
            if (v is long l) return l;
            if (v is ulong ul) return unchecked((long)ul);
            if (v is int i) return i;
            if (v is uint ui) return ui;
            if (v is short s) return s;
            if (v is ushort us) return us;
            if (v is byte b) return b;
            if (v is sbyte sb) return sb;
            try { return Convert.ToInt64(v); } catch { return null; }
        }

        static ulong? GetULong(object o, string name)
        {
            var v = GetField(o, name);
            if (v == null) return null;
            if (v is ulong ul) return ul;
            if (v is long l) return unchecked((ulong)l);
            if (v is int i) return (ulong)Math.Max(0, (long)i);
            if (v is uint ui) return ui;
            try { return Convert.ToUInt64(v); } catch { return null; }
        }

        static void SetField(object o, string name, object value)
        {
            var f = FindField(o, name);
            if (f == null) return;
            try
            {
                if (value == null || f.FieldType == value.GetType())
                {
                    f.SetValue(o, value);
                    return;
                }
                f.SetValue(o, Convert.ChangeType(value, f.FieldType));
            }
            catch (Exception)
            {
                try
                {
                    var t = f.FieldType;
                    if (t == typeof(int) || t == typeof(uint) || t == typeof(short) || t == typeof(ushort)
                        || t == typeof(byte) || t == typeof(sbyte) || t == typeof(long) || t == typeof(ulong))
                        f.SetValue(o, Convert.ChangeType(value, t));
                }
                catch { }
            }
        }

        static void SetLong(object o, string name, long value)
        {
            SetField(o, name, value);
        }
    }
}
