import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ResponsiveContainer, AreaChart, XAxis, YAxis, Tooltip, Area } from 'recharts';

function App() {
  const [stats, setStats] = useState({
    cpu: 0,
    ram: 0,
    temp: 0,
    ramTotal: 0,
  });
  const [log, setLog] = useState('');
  const [logType, setLogType] = useState<'success' | 'error' | 'info'>('info');
  const [isCleaningTemp, setIsCleaningTemp] = useState(false);
  const [isDisablingTelemetry, setIsDisablingTelemetry] = useState(false);
  const [isEnablingGamerMode, setIsEnablingGamerMode] = useState(false);
  const [isGamerModeOn, setIsGamerModeOn] = useState(false);
  const [isOptimizingNetwork, setIsOptimizingNetwork] = useState(false);
  const [isNuking, setIsNuking] = useState(false);
  const [nukeLog, setNukeLog] = useState('');
  const [nukeLogType, setNukeLogType] = useState<'success' | 'error' | 'info'>('info');
  const [startupApps, setStartupApps] = useState<string[]>([]);
  const [history, setHistory] = useState<any[]>([]);

  const fetchStartupApps = async () => {
    try {
      const apps: string[] = await invoke('get_startup_apps');
      setStartupApps(apps);
    } catch (error) {
      console.error('Failed to fetch startup apps:', error);
    }
  };

  useEffect(() => {
    fetchStartupApps();
  }, []);

  useEffect(() => {
    const fetchStats = async () => {
      try {
        const result: any = await invoke('get_system_stats');
        const ramUsedGB = Number((result.used_memory_mb / 1024).toFixed(1));
        const ramTotalGB = Number((result.total_memory_mb / 1024).toFixed(1));

        setStats({
          cpu: Math.round(result.cpu_overall_percent || 0),
          ram: ramUsedGB,
          temp: Math.round(
            result.temperatures && result.temperatures.length > 0
              ? result.temperatures[0].temperature_celsius
              : 0
          ),
          ramTotal: ramTotalGB,
        });

        setHistory(prev => {
          const newStat = {
            time: new Date().toLocaleTimeString([], { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' }),
            cpu: Math.round(result.cpu_overall_percent || 0),
            ram: ramUsedGB,
          };
          return [...prev.slice(-19), newStat];
        });
      } catch (error) {
        console.error('Backend connection failed:', error);
      }
    };

    fetchStats();
    const interval = setInterval(fetchStats, 1000);
    return () => clearInterval(interval);
  }, []);

  const handleCleanTemp = async () => {
    setIsCleaningTemp(true);
    setLog('Scanning temporary files...');
    setLogType('info');
    try {
      const result: string = await invoke('clean_temp_files');
      setLog(result);
      setLogType('success');
    } catch (error: any) {
      setLog(error?.toString() || 'An unknown error occurred.');
      setLogType('error');
    } finally {
      setIsCleaningTemp(false);
    }
  };

  const handleDisableTelemetry = async () => {
    setIsDisablingTelemetry(true);
    setLog('Modifying registry keys...');
    setLogType('info');
    try {
      const result: string = await invoke('disable_telemetry');
      setLog(result);
      setLogType('success');
    } catch (error: any) {
      setLog(error?.toString() || 'An unknown error occurred.');
      setLogType('error');
    } finally {
      setIsDisablingTelemetry(false);
    }
  };

  const handleToggleGamerMode = async () => {
    setIsEnablingGamerMode(true);
    if (isGamerModeOn) {
      setLog('Deactivating Gamer Mode...');
      setLogType('info');
      try {
        const result: string = await invoke('disable_gamer_mode');
        setLog(result);
        setLogType('success');
        setIsGamerModeOn(false);
      } catch (error: any) {
        setLog(error?.toString() || 'An unknown error occurred.');
        setLogType('error');
      } finally {
        setIsEnablingGamerMode(false);
      }
    } else {
      setLog('Activating Gamer Mode...');
      setLogType('info');
      try {
        const result: string = await invoke('enable_gamer_mode');
        setLog(result);
        setLogType('success');
        setIsGamerModeOn(true);
      } catch (error: any) {
        setLog(error?.toString() || 'An unknown error occurred.');
        setLogType('error');
      } finally {
        setIsEnablingGamerMode(false);
      }
    }
  };

  const handleOptimizeNetwork = async () => {
    setIsOptimizingNetwork(true);
    setLog('Optimizing network stack...');
    setLogType('info');
    try {
      const result: string = await invoke('optimize_network');
      setLog(result);
      setLogType('success');
    } catch (error: any) {
      setLog(error?.toString() || 'An unknown error occurred.');
      setLogType('error');
    } finally {
      setIsOptimizingNetwork(false);
    }
  };

  const handleNukeSystem = async () => {
    setIsNuking(true);
    setNukeLog('Initiating full system nuke sequence...');
    setNukeLogType('info');
    try {
      const result: string = await invoke('nuke_system');
      setNukeLog(result);
      setNukeLogType('success');
    } catch (error: any) {
      setNukeLog(error?.toString() || 'An unknown error occurred.');
      setNukeLogType('error');
    } finally {
      setIsNuking(false);
    }
  };

  const handleDisableStartupApp = async (appName: string) => {
    try {
      await invoke('disable_startup_app', { appName });
      await fetchStartupApps();
    } catch (error) {
      console.error('Failed to disable startup app:', error);
    }
  };

  const logBorderColor =
    logType === 'success'
      ? 'border-green-500/50'
      : logType === 'error'
        ? 'border-red-500/50'
        : 'border-cyan-500/30';
  const logTextColor =
    logType === 'success'
      ? 'text-green-400'
      : logType === 'error'
        ? 'text-red-400'
        : 'text-cyan-300';

  const nukeLogBorderColor =
    nukeLogType === 'success'
      ? 'border-green-500/50'
      : nukeLogType === 'error'
        ? 'border-red-500/50'
        : 'border-red-500/30';
  const nukeLogTextColor =
    nukeLogType === 'success'
      ? 'text-green-400'
      : nukeLogType === 'error'
        ? 'text-red-400'
        : 'text-orange-300';

  return (
    <div className="min-h-screen bg-[#0d0d0f] text-gray-200 p-8 font-mono">
      {/* Header */}
      <header className="mb-8 border-b border-cyan-500/30 pb-4">
        <h1 className="text-3xl font-bold text-cyan-400 tracking-wider flex items-center gap-3">
          ⚡ SYSFORGE{' '}
          <span className="text-xs text-gray-500 border border-gray-700 px-2 py-1 rounded bg-black">
            v0.4.0 Pro
          </span>
        </h1>
        <p className="text-gray-400 text-sm mt-2">
          Advanced Windows Optimizer and De-bloater
        </p>
      </header>

      {/* Stats Cards */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
        <div className="bg-black/40 border border-gray-800/80 rounded-xl p-6 shadow-[0_0_15px_rgba(34,211,238,0.03)] hover:border-cyan-500/50 transition-colors">
          <h2 className="text-cyan-500 text-sm font-semibold tracking-widest mb-3">
            CPU USAGE
          </h2>
          <div className="text-5xl font-light">
            {stats.cpu} <span className="text-2xl text-gray-500">%</span>
          </div>
        </div>

        <div className="bg-black/40 border border-gray-800/80 rounded-xl p-6 shadow-[0_0_15px_rgba(34,211,238,0.03)] hover:border-cyan-500/50 transition-colors">
          <h2 className="text-cyan-500 text-sm font-semibold tracking-widest mb-3">
            RAM USAGE
          </h2>
          <div className="text-5xl font-light">
            {stats.ram} <span className="text-2xl text-gray-500">GB</span>
          </div>
          <div className="text-xs text-gray-600 mt-2">
            Total: {stats.ramTotal} GB
          </div>
        </div>

        <div className="bg-black/40 border border-gray-800/80 rounded-xl p-6 shadow-[0_0_15px_rgba(34,211,238,0.03)] hover:border-cyan-500/50 transition-colors">
          <h2 className="text-cyan-500 text-sm font-semibold tracking-widest mb-3">
            SYSTEM TEMP
          </h2>
          <div className="text-5xl font-light">
            {stats.temp} <span className="text-2xl text-gray-500">°C</span>
          </div>
        </div>
      </div>

      {/* Live Analytics */}
      <div className="mt-10">
        <h2 className="text-lg font-semibold text-cyan-400 tracking-widest mb-4 border-b border-cyan-500/20 pb-2">
          LIVE ANALYTICS
        </h2>
        <div className="bg-black/40 border border-gray-800/80 rounded-xl p-6 shadow-[0_0_15px_rgba(34,211,238,0.03)] h-72 relative">
          <div className="absolute top-4 right-6 flex gap-4 text-xs font-semibold tracking-wider">
            <span className="flex items-center gap-2"><span className="w-2 h-2 rounded-full bg-cyan-500 shadow-[0_0_8px_rgba(6,182,212,0.8)]"></span> CPU</span>
            <span className="flex items-center gap-2"><span className="w-2 h-2 rounded-full bg-purple-500 shadow-[0_0_8px_rgba(139,92,246,0.8)]"></span> RAM</span>
          </div>
          <ResponsiveContainer width="100%" height="100%">
            <AreaChart data={history} margin={{ top: 20, right: 0, left: -20, bottom: 0 }}>
              <defs>
                <linearGradient id="colorCpu" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="5%" stopColor="#06b6d4" stopOpacity={0.4} />
                  <stop offset="95%" stopColor="#06b6d4" stopOpacity={0} />
                </linearGradient>
                <linearGradient id="colorRam" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="5%" stopColor="#8b5cf6" stopOpacity={0.4} />
                  <stop offset="95%" stopColor="#8b5cf6" stopOpacity={0} />
                </linearGradient>
                <filter id="glow">
                  <feGaussianBlur stdDeviation="2" result="coloredBlur"/>
                  <feMerge>
                    <feMergeNode in="coloredBlur"/>
                    <feMergeNode in="SourceGraphic"/>
                  </feMerge>
                </filter>
              </defs>
              <XAxis dataKey="time" stroke="#374151" fontSize={10} tickLine={false} axisLine={false} minTickGap={30} />
              <YAxis stroke="#374151" fontSize={10} tickLine={false} axisLine={false} />
              <Tooltip
                contentStyle={{ backgroundColor: 'rgba(0,0,0,0.9)', border: '1px solid currentColor', borderRadius: '0.5rem', fontFamily: 'monospace', fontSize: '12px' }}
                itemStyle={{ color: '#e5e7eb', textTransform: 'uppercase' }}
                labelStyle={{ color: '#9ca3af', marginBottom: '4px' }}
              />
              <Area type="monotone" dataKey="cpu" stroke="#06b6d4" strokeWidth={2} fillOpacity={1} fill="url(#colorCpu)" isAnimationActive={false} style={{ filter: 'url(#glow)' }} />
              <Area type="monotone" dataKey="ram" stroke="#8b5cf6" strokeWidth={2} fillOpacity={1} fill="url(#colorRam)" isAnimationActive={false} style={{ filter: 'url(#glow)' }} />
            </AreaChart>
          </ResponsiveContainer>
        </div>
      </div>

      {/* Action Center */}
      <div className="mt-10">
        <h2 className="text-lg font-semibold text-cyan-400 tracking-widest mb-4 border-b border-cyan-500/20 pb-2">
          ACTION CENTER
        </h2>

        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          {/* Clean Temp Files */}
          <button
            onClick={handleCleanTemp}
            disabled={isCleaningTemp}
            className="group bg-black/40 border border-gray-800/80 rounded-xl p-5 text-left
                       hover:border-cyan-500/50 hover:shadow-[0_0_20px_rgba(34,211,238,0.06)]
                       transition-all duration-200 disabled:opacity-50 disabled:cursor-not-allowed
                       active:scale-[0.98]"
          >
            <div className="flex items-center gap-3 mb-2">
              <span className="text-2xl">🧹</span>
              <h3 className="text-cyan-500 text-sm font-semibold tracking-widest">
                CLEAN TEMP FILES
              </h3>
            </div>
            <p className="text-gray-500 text-xs">
              {isCleaningTemp
                ? 'Cleaning in progress...'
                : 'Remove junk from %TEMP%, Prefetch & Windows Temp'}
            </p>
          </button>

          {/* Disable Telemetry */}
          <button
            onClick={handleDisableTelemetry}
            disabled={isDisablingTelemetry}
            className="group bg-black/40 border border-gray-800/80 rounded-xl p-5 text-left
                       hover:border-amber-500/50 hover:shadow-[0_0_20px_rgba(245,158,11,0.06)]
                       transition-all duration-200 disabled:opacity-50 disabled:cursor-not-allowed
                       active:scale-[0.98]"
          >
            <div className="flex items-center gap-3 mb-2">
              <span className="text-2xl">🛡️</span>
              <h3 className="text-amber-500 text-sm font-semibold tracking-widest">
                DISABLE TELEMETRY
              </h3>
            </div>
            <p className="text-gray-500 text-xs">
              {isDisablingTelemetry
                ? 'Modifying registry...'
                : 'Disable DiagTrack, data collection & telemetry services (requires Admin)'}
            </p>
          </button>

          {/* Optimize Network */}
          <button
            onClick={handleOptimizeNetwork}
            disabled={isOptimizingNetwork}
            className="group bg-black/40 border border-gray-800/80 rounded-xl p-5 text-left
                       hover:border-emerald-500/50 hover:shadow-[0_0_20px_rgba(16,185,129,0.06)]
                       transition-all duration-200 disabled:opacity-50 disabled:cursor-not-allowed
                       active:scale-[0.98]"
          >
            <div className="flex items-center gap-3 mb-2">
              <span className="text-2xl">🌐</span>
              <h3 className="text-emerald-500 text-sm font-semibold tracking-widest">
                OPTIMIZE NETWORK
              </h3>
            </div>
            <p className="text-gray-500 text-xs">
              {isOptimizingNetwork
                ? 'Resetting network stack...'
                : 'Flush DNS, reset Winsock & TCP/IP stack for lower ping'}
            </p>
          </button>
        </div>
      </div>

      {/* Performance Boost */}
      <div className="mt-10">
        <h2 className="text-lg font-semibold text-cyan-400 tracking-widest mb-4 border-b border-cyan-500/20 pb-2">
          PERFORMANCE BOOST
        </h2>

        <button
          onClick={handleToggleGamerMode}
          disabled={isEnablingGamerMode}
          className={`w-full group bg-black/40 border rounded-xl p-6 text-center
                     transition-all duration-300 disabled:opacity-50 disabled:cursor-not-allowed
                     active:scale-[0.98] relative overflow-hidden ${isGamerModeOn
              ? 'border-green-500/50 shadow-[0_0_25px_rgba(34,197,94,0.2)]'
              : 'border-cyan-500/30 hover:border-cyan-400 hover:shadow-[0_0_25px_rgba(34,211,238,0.3)]'
            }`}
        >
          <div className={`absolute inset-0 transition-opacity duration-300 ${isGamerModeOn
              ? 'bg-green-500/5 opacity-100'
              : 'bg-cyan-500/5 opacity-0 group-hover:opacity-100'
            }`} />
          <h3 className={`text-xl font-bold tracking-[0.2em] mb-2 relative z-10 flex items-center justify-center gap-3 ${isGamerModeOn ? 'text-green-400' : 'text-cyan-400'
            }`}>
            <span className="text-3xl">{isGamerModeOn ? '🟢' : '🎮'}</span>
            {isGamerModeOn ? 'GAMER MODE ACTIVE (CLICK TO DEACTIVATE)' : 'ACTIVATE GAMER MODE'}
          </h3>
          <p className={`text-sm relative z-10 ${isGamerModeOn ? 'text-green-600' : 'text-cyan-600'
            }`}>
            {isEnablingGamerMode
              ? (isGamerModeOn ? 'Restoring balanced mode...' : 'Optimizing system performance...')
              : (isGamerModeOn
                ? 'High Performance plan is active — click to restore Balanced mode'
                : 'Kill non-essential processes and enable High Performance power plan')}
          </p>
        </button>
      </div>

      {/* Log Output */}
      {log && (
        <div
          className={`mt-6 bg-black/60 border ${logBorderColor} rounded-xl p-4 transition-all duration-300`}
        >
          <h3 className="text-gray-500 text-xs font-semibold tracking-widest mb-2">
            OUTPUT LOG
          </h3>
          <pre className={`${logTextColor} text-sm whitespace-pre-wrap leading-relaxed`}>
            {log}
          </pre>
        </div>
      )}

      {/* ULTIMATE PROTOCOL */}
      <div className="mt-10">
        <h2 className="text-lg font-semibold text-red-400 tracking-widest mb-4 border-b border-red-500/20 pb-2">
          ULTIMATE PROTOCOL
        </h2>

        <button
          onClick={handleNukeSystem}
          disabled={isNuking}
          className="w-full group bg-black/60 border-2 border-red-900/60 rounded-xl p-8 text-center
                     hover:border-red-500/80 hover:shadow-[0_0_40px_rgba(220,38,38,0.15),inset_0_0_60px_rgba(220,38,38,0.05)]
                     transition-all duration-500 disabled:opacity-50 disabled:cursor-not-allowed
                     active:scale-[0.98] relative overflow-hidden"
        >
          {/* Animated background pulse */}
          <div className="absolute inset-0 bg-gradient-to-r from-red-950/0 via-red-900/10 to-red-950/0 opacity-0 group-hover:opacity-100 transition-opacity duration-500" />
          <div className="absolute inset-0 border border-red-500/10 rounded-xl group-hover:border-red-500/20 transition-colors duration-500" />

          <h3 className="text-red-400 text-2xl font-bold tracking-[0.3em] mb-3 relative z-10 flex items-center justify-center gap-4">
            <span className="text-4xl">☢️</span> INITIATE SYSTEM NUKE
          </h3>
          <p className="text-red-700 text-sm relative z-10 max-w-lg mx-auto">
            {isNuking
              ? '⏳ Running full optimization sequence... This may take a moment.'
              : 'Execute ALL optimizations in sequence — Clean, Kill Telemetry, Gamer Mode & Network Reset'}
          </p>
          <div className="mt-3 flex items-center justify-center gap-2 relative z-10">
            <span className="inline-block w-2 h-2 rounded-full bg-red-500/60 animate-pulse" />
            <span className="text-red-800 text-xs tracking-widest uppercase">
              {isNuking ? 'Nuking in progress' : 'Full system overhaul'}
            </span>
            <span className="inline-block w-2 h-2 rounded-full bg-red-500/60 animate-pulse" />
          </div>
        </button>

        {/* Nuke Output Log */}
        {nukeLog && (
          <div
            className={`mt-4 bg-black/70 border ${nukeLogBorderColor} rounded-xl p-5 transition-all duration-300`}
          >
            <h3 className="text-red-500/80 text-xs font-semibold tracking-widest mb-3 flex items-center gap-2">
              <span className="inline-block w-1.5 h-1.5 rounded-full bg-red-500 animate-pulse" />
              NUKE REPORT
            </h3>
            <pre className={`${nukeLogTextColor} text-sm whitespace-pre-wrap leading-relaxed font-mono`}>
              {nukeLog}
            </pre>
          </div>
        )}

        {/* BOOT OPTIMIZER - Phase 5 */}
        <div className="mt-12">
          <h2 className="text-lg font-semibold text-cyan-400 tracking-widest mb-4 border-b border-cyan-500/20 pb-2">
            BOOT OPTIMIZER
          </h2>
          <div className="bg-black/40 border border-gray-800/80 rounded-xl p-6 shadow-[0_0_15px_rgba(34,211,238,0.03)]">
            <p className="text-gray-400 text-sm mb-6">
              Kill background apps that slow down your PC startup.
            </p>

            <div className="flex flex-col gap-2">
              {startupApps.length === 0 ? (
                <div className="text-gray-500 text-sm italic py-4 text-center border border-gray-800/50 rounded-lg bg-black/20">
                  No startup apps found. Your boot sequence is optimized.
                </div>
              ) : (
                startupApps.map((app) => (
                  <div key={app} className="flex justify-between items-center bg-black/40 border border-gray-800 rounded-lg p-3 hover:border-gray-700 transition-colors">
                    <span className="text-gray-200 font-mono text-sm">{app}</span>
                    <button
                      onClick={() => handleDisableStartupApp(app)}
                      className="text-red-500 border border-red-500/30 px-3 py-1 rounded text-xs font-bold tracking-wider hover:bg-red-500/10 hover:border-red-500 transition-all focus:outline-none focus:ring-2 focus:ring-red-500/50 active:scale-[0.98]"
                    >
                      DISABLE
                    </button>
                  </div>
                ))
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export default App;
