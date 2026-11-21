using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;

public static class Pro9i120Display {
    [StructLayout(LayoutKind.Sequential)] private struct PowerStatus {
        public byte acLine, batteryFlag, batteryPercent, systemStatus;
        public uint batteryLifeTime, batteryFullLifeTime;
    }
    [DllImport("kernel32.dll")] private static extern bool GetSystemPowerStatus(out PowerStatus status);
    public static bool OnAC() { PowerStatus status; return GetSystemPowerStatus(out status) && status.acLine == 1; }
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    public struct Device {
        public uint cb;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string name;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)] public string description;
        public uint flags;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)] public string id;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)] public string key;
    }
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    public struct Mode {
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string deviceName;
        public ushort specVersion, driverVersion, size, driverExtra;
        public uint fields;
        public int x, y;
        public uint orientation, fixedOutput;
        public short color, duplex, yResolution, ttOption, collate;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string formName;
        public ushort logPixels;
        public uint bitsPerPixel, width, height, displayFlags, hz;
        public uint icmMethod, icmIntent, mediaType, ditherType, reserved1, reserved2, panningWidth, panningHeight;
    }
    [StructLayout(LayoutKind.Sequential)] public struct Luid { public uint low; public int high; }
    [StructLayout(LayoutKind.Sequential)] public struct Rational { public uint numerator, denominator; }
    [StructLayout(LayoutKind.Sequential)] public struct Source { public Luid adapter; public uint id, modeIndex, flags; }
    [StructLayout(LayoutKind.Sequential)] public struct Target {
        public Luid adapter;
        public uint id, modeIndex, technology, rotation, scaling;
        public Rational refresh;
        public uint scanlineOrdering;
        public int available;
        public uint flags;
    }
    [StructLayout(LayoutKind.Sequential)] public struct Path { public Source source; public Target target; public uint flags; }
    public class DeviceModes { public Device device; public Mode current; public Mode registry; public List<Mode> compatible; public List<Mode> raw; }
    public class ActivePath {
        public Path path;
        public string sourceName, targetName, devicePath;
        public int sourceNameResult, targetNameResult, colorResult, whiteResult;
        public uint advancedColorFlags, colorEncoding, bitsPerColorChannel, sdrWhiteLevel;
        public ulong pixelRate;
        public uint signalWidth, signalHeight, totalWidth, totalHeight;
        public Rational signalVertical;
    }
    public class Snapshot { public List<ActivePath> active; public string modesBase64; public int modeCount; }
    [DllImport("user32.dll", CharSet=CharSet.Unicode, ExactSpelling=true)]
    private static extern bool EnumDisplayDevicesW(string name, uint index, ref Device device, uint flags);
    [DllImport("user32.dll", CharSet=CharSet.Unicode, ExactSpelling=true)]
    private static extern bool EnumDisplaySettingsExW(string name, uint index, ref Mode mode, uint flags);
    [DllImport("user32.dll")] private static extern int GetDisplayConfigBufferSizes(uint flags, out uint paths, out uint modes);
    [DllImport("user32.dll")] private static extern int QueryDisplayConfig(uint flags, ref uint paths, [Out] Path[] pathArray, ref uint modes, IntPtr modeArray, IntPtr topology);
    [DllImport("user32.dll")] private static extern int DisplayConfigGetDeviceInfo(IntPtr packet);
    private static Mode NewMode() { return new Mode { size = (ushort)Marshal.SizeOf(typeof(Mode)) }; }
    private static List<Mode> Enumerate(string name, uint flags) {
        var result = new List<Mode>();
        for(uint index=0; index<10000; index++) {
            var mode = NewMode();
            if(!EnumDisplaySettingsExW(name,index,ref mode,flags)) return result;
            result.Add(mode);
        }
        throw new Exception("Display mode enumeration exceeded safety bound");
    }
    public static List<DeviceModes> ReadModes() {
        if(Marshal.SizeOf(typeof(Mode)) != 220 || Marshal.SizeOf(typeof(Path)) != 72) throw new Exception("Unexpected Windows struct sizes");
        var result = new List<DeviceModes>();
        for(uint i=0; i<64; i++) {
            var device = new Device { cb=(uint)Marshal.SizeOf(typeof(Device)) };
            if(!EnumDisplayDevicesW(null,i,ref device,0)) break;
            if((device.flags & 1)==0 || (device.flags & 8)!=0) continue;
            var current=NewMode(); var registry=NewMode();
            if(!EnumDisplaySettingsExW(device.name,0xffffffff,ref current,0)) throw new Exception("Cannot read current mode: "+device.name);
            EnumDisplaySettingsExW(device.name,0xfffffffe,ref registry,0);
            result.Add(new DeviceModes { device=device, current=current, registry=registry, compatible=Enumerate(device.name,0), raw=Enumerate(device.name,2) });
        }
        return result;
    }
    private static IntPtr Packet(int type,int size,Luid adapter,uint id) {
        var p=Marshal.AllocHGlobal(size);
        Marshal.Copy(new byte[size],0,p,size);
        Marshal.WriteInt32(p,0,type); Marshal.WriteInt32(p,4,size);
        Marshal.WriteInt32(p,8,unchecked((int)adapter.low)); Marshal.WriteInt32(p,12,adapter.high); Marshal.WriteInt32(p,16,unchecked((int)id));
        return p;
    }
    public static Snapshot ReadActive() {
        for(int retry=0;retry<4;retry++) {
            uint pc,mc;
            int rc=GetDisplayConfigBufferSizes(2,out pc,out mc);
            if(rc!=0) throw new Exception("GetDisplayConfigBufferSizes: "+rc);
            var paths=new Path[pc]; var modes=Marshal.AllocHGlobal(checked((int)mc*64));
            Marshal.Copy(new byte[checked((int)mc*64)],0,modes,checked((int)mc*64));
            try {
                rc=QueryDisplayConfig(2,ref pc,paths,ref mc,modes,IntPtr.Zero);
                if(rc==122) continue;
                if(rc!=0) throw new Exception("QueryDisplayConfig: "+rc);
                var result=new List<ActivePath>();
                for(int i=0;i<pc;i++) {
                    var path=paths[i]; var item=new ActivePath { path=path };
                    var src=Packet(1,84,path.source.adapter,path.source.id);
                    var tgt=Packet(2,420,path.target.adapter,path.target.id);
                    var color=Packet(9,32,path.target.adapter,path.target.id);
                    var white=Packet(11,24,path.target.adapter,path.target.id);
                    try {
                        item.sourceNameResult=DisplayConfigGetDeviceInfo(src);
                        if(item.sourceNameResult==0) item.sourceName=Marshal.PtrToStringUni(IntPtr.Add(src,20));
                        item.targetNameResult=DisplayConfigGetDeviceInfo(tgt);
                        if(item.targetNameResult==0) { item.targetName=Marshal.PtrToStringUni(IntPtr.Add(tgt,36)); item.devicePath=Marshal.PtrToStringUni(IntPtr.Add(tgt,164)); }
                        item.colorResult=DisplayConfigGetDeviceInfo(color);
                        if(item.colorResult==0) { item.advancedColorFlags=(uint)Marshal.ReadInt32(color,20); item.colorEncoding=(uint)Marshal.ReadInt32(color,24); item.bitsPerColorChannel=(uint)Marshal.ReadInt32(color,28); }
                        item.whiteResult=DisplayConfigGetDeviceInfo(white);
                        if(item.whiteResult==0) item.sdrWhiteLevel=(uint)Marshal.ReadInt32(white,20);
                        if(path.target.modeIndex < mc) {
                            var m=IntPtr.Add(modes,checked((int)path.target.modeIndex*64));
                            if(Marshal.ReadInt32(m,0)==2) {
                                item.pixelRate=unchecked((ulong)Marshal.ReadInt64(m,16));
                                item.signalVertical=new Rational { numerator=(uint)Marshal.ReadInt32(m,32), denominator=(uint)Marshal.ReadInt32(m,36) };
                                item.signalWidth=(uint)Marshal.ReadInt32(m,40); item.signalHeight=(uint)Marshal.ReadInt32(m,44);
                                item.totalWidth=(uint)Marshal.ReadInt32(m,48); item.totalHeight=(uint)Marshal.ReadInt32(m,52);
                            }
                        }
                        result.Add(item);
                    } finally { Marshal.FreeHGlobal(src); Marshal.FreeHGlobal(tgt); Marshal.FreeHGlobal(color); Marshal.FreeHGlobal(white); }
                }
                var bytes=new byte[checked((int)mc*64)]; Marshal.Copy(modes,bytes,0,bytes.Length);
                return new Snapshot {active=result,modesBase64=Convert.ToBase64String(bytes),modeCount=(int)mc};
            } finally { Marshal.FreeHGlobal(modes); }
        }
        throw new Exception("Display topology was not stable");
    }
}
