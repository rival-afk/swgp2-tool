// SWGP2 Mod - in-game menu for Super Woden GP 2.
//
// Built by the game itself (Unity IMGUI), no DX hooks, no external windows.
// Injected via swgp2-mod (native loader). Toggle with the corner button or
// the header "—" button. F10 also toggles the menu when the game accepts
// legacy input.
//
// Field access on MainCar (Assembly-CSharp) goes through reflection so the
// mod survives game patches that only rename/shuffle implementation details.
// Missing fields are tolerated - the matching control just does nothing.
//
// Build (from the repo root, see mod\build.ps1):
//   csc.exe /target:library /nowarn Swgp2Mod.cs /r:<Managed>\mscorlib.dll ^
//     /r:UnityEngine.CoreModule.dll /r:UnityEngine.IMGUIModule.dll ^
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
        public static void Main()
        {
            try
            {
                var host = new GameObject("Swgp2Mod_Host");
                UnityEngine.Object.DontDestroyOnLoad(host);
                host.AddComponent<ModBehaviour>();
                Log("mod loaded");
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
        private bool _show = true;
        private bool _freezeLife;
        private bool _turbo;
        private float _powerBoost = 1.0f;      // x multiplier on CarPower
        private float _topSpeedBoost = 1.0f;   // x multiplier on CarTopSpeed
        private float _lifePct = 100f;         // 1..200 % of snapshot Life

        private object _car;                   // MainCar instance (cached)
        private ulong _lifeSnapshot = ulong.MaxValue;
        private long _powerSnapshot = long.MinValue;
        private long _topSpeedSnapshot = long.MinValue;

        // ---------------------------------------------------------- lifecycle
        void Update()
        {
            try
            {
                RefreshCar();
                if (_car != null && _freezeLife && _lifeSnapshot != ulong.MaxValue)
                    SetField(_car, "Life", _lifeSnapshot);
            }
            catch (Exception e) { Entry.Log("update: " + e.Message); }
        }

        void RefreshCar()
        {
            if (_car != null && ((UnityEngine.Object)_car) != null)
                return;
            _car = null;
            try
            {
                var objType = typeof(UnityEngine.Object);
                // non-generic overload avoids binding to a generic signature
                var m = objType.GetMethod("FindObjectOfType", new[] { typeof(Type) });
                if (m == null)
                {
                    // generic overload fallback: FindObjectOfType<MainCar>()
                    var carType = Type.GetType("MainCar, Assembly-CSharp");
                    if (carType == null) { Entry.Log("MainCar type not found"); return; }
                    var generic = objType.GetMethod("FindObjectOfType", new Type[0]);
                    if (generic != null)
                    {
                        var concrete = generic.MakeGenericMethod(carType);
                        _car = concrete.Invoke(null, null);
                    }
                }
                else
                {
                    _car = m.Invoke(null, new object[] { Type.GetType("MainCar, Assembly-CSharp") });
                }
            }
            catch (Exception e)
            {
                Entry.Log("car lookup failed: " + e.Message);
            }

            if (_car != null)
            {
                SnapshotOnce();
            }
        }

        void SnapshotOnce()
        {
            _lifeSnapshot = GetULong(_car, "Life") ?? ulong.MaxValue;
            _powerSnapshot = GetLong(_car, "CarPower") ?? long.MinValue;
            _topSpeedSnapshot = GetLong(_car, "CarTopSpeed") ?? long.MinValue;
        }

        // ---------------------------------------------------------- UI
        void OnGUI()
        {
            try
            {
                RefreshCar();
                if (!_show)
                {
                    // tiny always-available handle to re-open the menu
                    if (GUI.Button(new Rect(Screen.width - 96, 8, 88, 24), "SWGP2 ≡"))
                        _show = true;
                    return;
                }

                GUILayout.Window(0x5B6C, new Rect(Screen.width - 400, 30, 390, 360), DrawMenu, "SWGP2 Mod");
            }
            catch (Exception e)
            {
                Entry.Log("gui: " + e.Message);
                _show = false;
            }
        }

        void DrawMenu(int id)
        {
            GUILayout.BeginVertical();
            GUILayout.Space(2);

            if (_car == null)
            {
                GUILayout.Label("Car not found - start a race first.");
                GUILayout.Space(4);
                if (GUILayout.Button("Hide (F10)")) _show = false;
                GUILayout.EndVertical();
                GUI.DragWindow();
                return;
            }

            // --- status line
            string carId = GetField(_car, "CarId")?.ToString() ?? "?";
            GUILayout.Label("Car #" + carId + "   Life " + LifeText());
            GUILayout.Space(6);

            // --- power (acceleration)
            GUILayout.Label("Acceleration x" + _powerBoost.ToString("0.0"));
            _powerBoost = GUILayout.HorizontalSlider(_powerBoost, 1.0f, 5.0f);
            if (_powerSnapshot != long.MinValue && _powerSnapshot != 0)
                SetLong(_car, "CarPower", (long)(_powerSnapshot * _powerBoost));

            GUILayout.Space(6);
            GUILayout.Label("Top speed x" + _topSpeedBoost.ToString("0.0"));
            _topSpeedBoost = GUILayout.HorizontalSlider(_topSpeedBoost, 1.0f, 3.0f);
            if (_topSpeedSnapshot != long.MinValue && _topSpeedSnapshot != 0)
                SetLong(_car, "CarTopSpeed", (long)(_topSpeedSnapshot * _topSpeedBoost));

            GUILayout.Space(6);

            // --- durability
            bool prevFreeze = _freezeLife;
            _freezeLife = GUILayout.Toggle(_freezeLife, "Freeze durability");
            if (_freezeLife != prevFreeze)
            {
                if (_freezeLife && _car != null)
                    _lifeSnapshot = GetULong(_car, "Life") ?? _lifeSnapshot;
                Entry.Log(_freezeLife ? "durability frozen" : "durability unfrozen");
            }

            GUILayout.Label("Durability " + _lifePct.ToString("0") + "%");
            _lifePct = GUILayout.HorizontalSlider(_lifePct, 1f, 200f);
            if (!_freezeLife && _lifeSnapshot != ulong.MaxValue)
            {
                ulong target = (ulong)((double)_lifeSnapshot * (_lifePct / 100.0));
                SetField(_car, "Life", target);
            }

            if (GUILayout.Button("Repair to full"))
            {
                _lifePct = 200f;
                _freezeLife = true;
                if (_car != null)
                    _lifeSnapshot = GetULong(_car, "Life") ?? _lifeSnapshot;
            }

            GUILayout.Space(6);

            // --- extras
            _turbo = GUILayout.Toggle(_turbo, "Turbo");
            SetField(_car, "Turbo", _turbo);
            if (GUILayout.Button("Refill fuel"))
            {
                var f = GetLong(_car, "Fuel") ?? 0;
                SetLong(_car, "Fuel", f + 5000);
            }

            GUILayout.Space(8);
            if (GUILayout.Button("Hide (F10)")) _show = false;
            GUILayout.EndVertical();
            GUI.DragWindow();
        }

        // -------------------------------------------------- field access glue
        static FieldInfo FindField(object o, string name)
        {
            var t = o.GetType();
            while (t != null)
            {
                var f = t.GetField(name, BindingFlags.Public | BindingFlags.Instance | BindingFlags.NonPublic | BindingFlags.FlattenHierarchy);
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
                // type mismatch - the field may be an enum or a different width;
                // try a numeric coercion for integral-like fields
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

        string LifeText()
        {
            var v = GetULong(_car, "Life");
            return v.HasValue ? v.Value.ToString() : "?";
        }
    }
}
