using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;

[assembly: DefaultDllImportSearchPaths(DllImportSearchPath.System32)]

// Calls the installed Intel Control Library using READ selectors only.
// No Intel DLL, driver or factory EDID is distributed with this helper.
public static class Pro9i120Intel {
    const string Dll = "ControlLib.dll";
    [StructLayout(LayoutKind.Sequential)] struct Init {
        public uint Size; public byte Version;
        public uint AppVersion, Flags, SupportedVersion; public Guid ApplicationUID;
    }
    [StructLayout(LayoutKind.Explicit, Size=16)] struct EncoderId {
        [FieldOffset(0)] public uint WindowsId;
        [FieldOffset(0)] public IntPtr Pointer;
        [FieldOffset(8)] public uint PointerSize;
    }
    [StructLayout(LayoutKind.Sequential)] struct Revision { public byte Major, Minor, RevisionNumber; }
    [StructLayout(LayoutKind.Sequential)] struct Timing {
        public uint Size; public byte Version; public ulong PixelClock;
        public uint HActive,VActive,HTotal,VTotal,HBlank,VBlank,HSync,VSync;
        public float RefreshRate; public uint SignalStandard; public byte VicId;
    }
    [StructLayout(LayoutKind.Sequential)] struct Properties {
        public uint Size; public byte Version; public EncoderId OsId;
        public uint Type,Mux,ConverterOutput; public Revision Spec;
        public uint BpcFlags,ConverterType,ConfigFlags,EnabledFeatures,SupportedFeatures,AdvancedEnabled,AdvancedSupported;
        public Timing Timing;
        [MarshalAs(UnmanagedType.ByValArray,SizeConst=16)] public uint[] Reserved;
    }
    [StructLayout(LayoutKind.Sequential)] struct Descriptor {
        public uint Size; public byte Version;
        public uint OpType,BlockNumber,DataSize; public IntPtr Data;
    }
    [DllImport(Dll,ExactSpelling=true)] static extern uint ctlInit(ref Init init,out IntPtr handle);
    [DllImport(Dll,ExactSpelling=true)] static extern uint ctlClose(IntPtr handle);
    [DllImport(Dll,ExactSpelling=true)] static extern uint ctlEnumerateDevices(IntPtr handle,ref uint count,[Out] IntPtr[] devices);
    [DllImport(Dll,ExactSpelling=true)] static extern uint ctlEnumerateDisplayOutputs(IntPtr device,ref uint count,[Out] IntPtr[] outputs);
    [DllImport(Dll,ExactSpelling=true)] static extern uint ctlGetDisplayProperties(IntPtr output,ref Properties properties);
    [DllImport(Dll,ExactSpelling=true)] static extern uint ctlPanelDescriptorAccess(IntPtr output,ref Descriptor descriptor);

    static void Check(uint result, string operation) {
        if(result != 0) throw new Exception(operation + " failed (0x" + result.ToString("X8") + "). No override installed.");
    }
    static byte[] ReadBlock(IntPtr output,uint number) {
        var descriptor=new Descriptor {Size=(uint)Marshal.SizeOf(typeof(Descriptor)),OpType=1,BlockNumber=number};
        Check(ctlPanelDescriptorAccess(output,ref descriptor),"Reading EDID block size");
        if(descriptor.DataSize!=128) throw new Exception("Expected a complete 128-byte EDID block.");
        descriptor.Data=Marshal.AllocHGlobal(128);
        try {
            Check(ctlPanelDescriptorAccess(output,ref descriptor),"Reading EDID block");
            if(descriptor.DataSize!=128) throw new Exception("EDID block length changed while reading.");
            var bytes=new byte[128];Marshal.Copy(descriptor.Data,bytes,0,128);return bytes;
        } finally {Marshal.FreeHGlobal(descriptor.Data);}
    }
    public static byte[] Read(uint windowsTargetId) {
        if(IntPtr.Size!=8) throw new Exception("Use 64-bit Windows PowerShell.");
        var init=new Init {Size=(uint)Marshal.SizeOf(typeof(Init)),AppVersion=0x00010000};
        IntPtr handle;Check(ctlInit(ref init,out handle),"Initializing installed Intel Control Library");
        try {
            uint count=0;Check(ctlEnumerateDevices(handle,ref count,null),"Enumerating Intel adapters");
            if(count==0 || count>16) throw new Exception("No suitable Intel adapter found.");
            var devices=new IntPtr[count];Check(ctlEnumerateDevices(handle,ref count,devices),"Reading Intel adapters");
            var matches=new List<IntPtr>();
            foreach(var device in devices) {
                uint outputCount=0;
                if(ctlEnumerateDisplayOutputs(device,ref outputCount,null)!=0 || outputCount>64) continue;
                var outputs=new IntPtr[outputCount];
                if(ctlEnumerateDisplayOutputs(device,ref outputCount,outputs)!=0) continue;
                foreach(var output in outputs) {
                    var properties=new Properties {Size=(uint)Marshal.SizeOf(typeof(Properties)),Reserved=new uint[16],Timing=new Timing {Size=(uint)Marshal.SizeOf(typeof(Timing))}};
                    if(ctlGetDisplayProperties(output,ref properties)==0 && properties.OsId.WindowsId==windowsTargetId) matches.Add(output);
                }
            }
            if(matches.Count!=1) throw new Exception("Cannot uniquely match the Windows panel to an Intel output.");
            var first=ReadBlock(matches[0],0);
            int blocks=first[126]+1;
            if(blocks!=3 && blocks!=4) throw new Exception("Unexpected EDID layout; no override installed.");
            var image=new byte[blocks*128];Array.Copy(first,image,128);
            for(uint block=1;block<blocks;block++) Array.Copy(ReadBlock(matches[0],block),0,image,block*128,128);
            return image;
        } finally {ctlClose(handle);}
    }
}
