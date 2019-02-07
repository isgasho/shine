﻿using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Runtime.InteropServices;
using UnityEngine;

namespace Shine
{
    public class LibraryInfo
    {
        public string Name { get; set; }
        public bool IsLoaded { get; set; }
    }

    public interface INativeLibrary
    {
        void Load();
        void Unload();

        LibraryInfo GetInfo();
    }

    public class NativeLibrary : INativeLibrary
    {
        public delegate IntPtr LoadLibraryDelegate(string name);
        public delegate void UnloadLibraryDelegate(IntPtr addr);
        public delegate IntPtr SymbolLookupDelegate(IntPtr addr, string name);

        public string LibraryPath { get; }

        protected IntPtr library_;
        protected LoadLibraryDelegate loadLibrary_;
        protected UnloadLibraryDelegate unloadLibrary_;
        protected SymbolLookupDelegate symbolLookup_;

        public NativeLibrary(string path, LoadLibraryDelegate loadLibrary, UnloadLibraryDelegate unloadLibrary, SymbolLookupDelegate getProcAddress)
        {
            LibraryPath = path;
            loadLibrary_ = loadLibrary;
            unloadLibrary_ = unloadLibrary;
            symbolLookup_ = getProcAddress;
        }

        public void Load()
        {
            Debug.Log($"Loading library {LibraryPath}");
            if (library_ != IntPtr.Zero)
            {
                Debug.Log($"Load library done, already loaded");
                return;
            }

            library_ = loadLibrary_(LibraryPath);
            Debug.Log($"Loading symbols...");
            LoadSymbols();
            Debug.Log($"Load library done.");
        }

        public void Unload()
        {
            Debug.Log($"Unloading library {LibraryPath}");
            if (library_ == IntPtr.Zero) {
                Debug.Log($"Unload library done, already unloaded.");
                return;
            }

            Debug.Log($"Unloading symbols...");
            UnloadSymbols();
            unloadLibrary_(library_);
            library_ = IntPtr.Zero;
            Debug.Log($"Unload library done.");
        }

        public LibraryInfo GetInfo()
        {
            return new LibraryInfo
            {
                Name = LibraryPath,
                IsLoaded = library_ != IntPtr.Zero,
            };
        }

        protected virtual void LoadSymbols() { }

        protected virtual void UnloadSymbols() { }
    }

    public class NativeLibrary<T> : NativeLibrary
        where T : class, new()
    {
        public T Api { get; }

        public NativeLibrary(string path, LoadLibraryDelegate loadLibrary, UnloadLibraryDelegate unloadLibrary, SymbolLookupDelegate getProcAddress)
            : base(path, loadLibrary, unloadLibrary, getProcAddress)
        {
            Api = new T();
        }

        protected override void LoadSymbols()
        {
            foreach (var prop in GetDelegates(typeof(T)))
            {
                Debug.Log($"Loading symbol {prop.Name} from {LibraryPath}");
                var callback = symbolLookup_(library_, prop.Name);
                if (callback == IntPtr.Zero)
                    throw new Exception($"Cannot load symbol: {prop.Name}");
                prop.SetValue(Api, Marshal.GetDelegateForFunctionPointer(callback, prop.FieldType));
            }
        }

        protected override void UnloadSymbols()
        {
            foreach (var prop in GetDelegates(typeof(T)))
            {
                Debug.Log($"Unloading symbol {prop.Name} from {LibraryPath}");
                prop.SetValue(Api, null);
            }
        }

        private IEnumerable<FieldInfo> GetDelegates(Type type)
        {
            return type.GetFields(BindingFlags.Instance | BindingFlags.Public);
        }
    }


    /// <summary>
    ///  Helper to load/unload libraries. It is safe to load/unload multiple lib from any thread but using the native library during load/unload
    ///  is undefined behavior.
    /// </summary>
    public static class NativeLoader
    {
        public const string DLL_PATH_PATTERN_NAME_MACRO = "{name}";
        public const string DLL_PATH_PATTERN_ASSETS_MACRO = "{assets}";
        public const string DLL_PATH_PATTERN_PROJECT_MACRO = "{proj}";

        public static string NativeLibraryPath { get; set; } =
#if UNITY_STANDALONE_WIN
            "{assets}/Plugin/{name}.dll";
#elif UNITY_STANDALONE_LINUX
            "{assets}/Plugins/{name}.so",
#endif

        public static T LoadNativeLibrary<T>(string libName)
            where T : class, new()
        {
            lock (libraries_)
            {
                INativeLibrary lib = null;
                if (!libraries_.TryGetValue(libName, out lib))
                {
                    lib = CreateNativeLibrary<T>(libName);
                    libraries_[libName] = lib;
                }
                lib.Load();
                var api = lib as NativeLibrary<T>;
                return api.Api;
            }
        }

        public static bool Any(Func<INativeLibrary, bool> pred)
        {
            lock (libraries_)
            {
                return libraries_.Values.Any(pred);
            }
        }

        public static void Foreach(Action<INativeLibrary> pred)
        {
            lock (libraries_)
            {
                foreach (var lib in libraries_.Values)
                {
                    pred(lib);
                }
            }
        }

        public static void LoadAll()
        {
            Debug.Log("Loading all libraries...");
            Foreach(x => x.Load());
            Debug.Log("Loading all libraries done.");
        }

        public static void UnloadAll()
        {
            Debug.Log("Unloading all libraries...");
            Foreach(x => x.Unload());
            Debug.Log("Unloading all libraries done.");
        }

        public static List<LibraryInfo> GetInfo()
        {
            var info = new List<LibraryInfo>();
            Foreach(x => info.Add(x.GetInfo()));
            return info;
        }

        private static Dictionary<string, INativeLibrary> libraries_ = new Dictionary<string, INativeLibrary> { };

        private static NativeLibrary<T> CreateNativeLibrary<T>(string libName)
            where T : class, new()
        {
            if (Environment.OSVersion.Platform.ToString().Contains("Win32"))
            {
                return CreateWindowsLibrary<T>(libName);
            }
            else if (Environment.OSVersion.Platform == PlatformID.Unix ||
                     Environment.OSVersion.Platform == PlatformID.MacOSX ||
                     (int)Environment.OSVersion.Platform == 128)
            {
                return CreatePosixLibrary<T>(libName);
            }

            throw new Exception("LoadLibrary failed: unknown OS");
        }


        private static string GetDllPath(string dllName)
        {
            return NativeLibraryPath
                .Replace(DLL_PATH_PATTERN_NAME_MACRO, dllName)
                .Replace(DLL_PATH_PATTERN_ASSETS_MACRO, Application.dataPath)
                .Replace(DLL_PATH_PATTERN_PROJECT_MACRO, Application.dataPath + "/../");
        }


        [DllImport("kernel32", SetLastError = true, CharSet = CharSet.Unicode)]
        private static extern IntPtr LoadLibrary(string lpFileName);

        [DllImport("kernel32", SetLastError = true, CharSet = CharSet.Unicode)]
        private static extern bool FreeLibrary(IntPtr hModule);

        [DllImport("kernel32.dll")]
        private static extern IntPtr GetProcAddress(IntPtr hModule, String procname);

        private static NativeLibrary<T> CreateWindowsLibrary<T>(string libName)
            where T : class, new()
        {
            string libFile = libName + ".dll";
            string rootDirectory = AppDomain.CurrentDomain.BaseDirectory;
            string assemblyDirectory = Path.GetDirectoryName(Assembly.GetExecutingAssembly().Location);

            var paths = new[] { GetDllPath(libName) };

            foreach (var path in paths)
            {
                if (path == null)
                    continue;

                if (File.Exists(path))
                    return new NativeLibrary<T>(path,
                        x =>
                        {
                            var handle = LoadLibrary(x);
                            if (handle == IntPtr.Zero)
                            {
                                var error = Marshal.GetLastWin32Error();
                                throw new Exception($"LoadLibrary failed: unable to load dll {x}: {string.Format("0x{0:x2}", error)}");
                            }
                            return handle;
                        },
                        x =>
                        {
                            var res = FreeLibrary(x);
                            if (!res)
                            {
                                var error = Marshal.GetLastWin32Error();
                                throw new Exception($"FreeLibrary failed: unable to unload dll {x}: {string.Format("0x{0:x2}", error)}");
                            }
                        },
                        GetProcAddress);
            }

            throw new Exception("LoadLibrary failed: unable to locate library " + libFile + ". Searched: " + paths.Aggregate((a, b) => a + "; " + b));
        }


        [DllImport("libdl.so")]
        private static extern IntPtr dlopen(string fileName, int flags);

        [DllImport("libdl.so")]
        private static extern int dlclose(IntPtr handle);

        [DllImport("libdl.so")]
        private static extern IntPtr dlerror();

        [DllImport("libdl.so")]
        private static extern IntPtr dlsym(IntPtr handle, string symbol);

        private static NativeLibrary<T> CreatePosixLibrary<T>(string libName)
            where T : class, new()
        {
            const int RTLD_NOW = 2;
            string libFile = "lib" + libName.ToLower() + ".so";
            string rootDirectory = AppDomain.CurrentDomain.BaseDirectory;
            string assemblyDirectory = Path.GetDirectoryName(Assembly.GetExecutingAssembly().Location);

            var paths = new[] { GetDllPath(libName) };

            foreach (var path in paths)
            {
                if (path == null)
                    continue;

                if (File.Exists(path))
                    return new NativeLibrary<T>(path,
                        x =>
                        {
                            var handle = dlopen(x, RTLD_NOW);
                            return handle;
                        },
                        x =>
                        {
                            if (dlclose(x) != 0)
                                throw new Exception($"dlclose failed: unable to unload dll {x}");
                        }
                        , dlsym);
            }

            throw new Exception("dlopen failed: unable to locate library " + libFile + ". Searched: " + paths.Aggregate((a, b) => a + "; " + b));
        }
    }
}
