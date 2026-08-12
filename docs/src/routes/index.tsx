import { baseOptions } from '@/lib/layout.shared';
import { createFileRoute, Link } from '@tanstack/react-router';
import { HomeLayout } from 'fumadocs-ui/layouts/home';
import { ArrowRight, Cpu, Layers, PlayCircle, ShieldCheck, Terminal, Zap } from 'lucide-react';

export const Route = createFileRoute('/')({
  component: Home,
});

function Home() {
  return (
    <HomeLayout {...baseOptions()}>
      <main className="flex-1 flex flex-col items-center overflow-x-hidden bg-black selection:bg-primary/30">

        {/* Animated Background Gradients & Orbs */}
        <div className="absolute inset-0 overflow-hidden pointer-events-none z-0">
          <div className="absolute -top-[20%] -left-[10%] w-[50%] h-[50%] rounded-full bg-emerald-500/20 blur-[150px] mix-blend-screen animate-pulse duration-10000" />
          <div className="absolute top-[30%] -right-[20%] w-[60%] h-[60%] rounded-full bg-indigo-500/20 blur-[150px] mix-blend-screen animate-pulse duration-7000 delay-1000" />
          <div className="absolute -bottom-[20%] left-[20%] w-[40%] h-[40%] rounded-full bg-teal-500/20 blur-[150px] mix-blend-screen animate-pulse duration-12000 delay-500" />

          {/* Subtle grid overlay */}
          <div className="absolute inset-0 bg-[linear-gradient(to_right,#80808012_1px,transparent_1px),linear-gradient(to_bottom,#80808012_1px,transparent_1px)] bg-[size:4rem_4rem] [mask-image:radial-gradient(ellipse_60%_50%_at_50%_0%,#000_70%,transparent_100%)]" />
        </div>

        {/* Hero Section */}
        <section className="relative w-full max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 pt-40 pb-32 flex flex-col items-center text-center z-10">
          <div className="animate-in fade-in slide-in-from-bottom-12 duration-1000 w-full max-w-4xl mx-auto flex flex-col items-center">

            <a href="/blog/v0-2-0-release" className="group inline-flex items-center rounded-full border border-white/10 bg-white/5 px-5 py-2 text-sm font-medium text-white mb-10 hover:bg-white/10 transition-all backdrop-blur-xl hover:scale-105 hover:shadow-[0_0_30px_-5px_rgba(52,211,153,0.3)]">
              <span className="flex h-2.5 w-2.5 rounded-full bg-emerald-400 mr-3 animate-pulse shadow-[0_0_10px_rgba(52,211,153,0.8)]" />
              Pace v0.2.0-rc.1 is now available
              <ArrowRight className="ml-2 w-4 h-4 text-emerald-400 group-hover:translate-x-1 transition-transform" />
            </a>

            <h1 className="text-6xl md:text-7xl lg:text-8xl font-extrabold tracking-tighter mb-8 leading-[1.1] text-white">
              Blazing <span className="text-transparent bg-clip-text bg-gradient-to-r from-emerald-400 to-teal-300">Speed.</span><br />
              Modern <span className="text-transparent bg-clip-text bg-gradient-to-r from-indigo-400 to-cyan-400">Ergonomics.</span>
            </h1>

            <p className="text-xl md:text-2xl text-gray-400 max-w-3xl mx-auto mb-12 font-medium leading-relaxed">
              A meticulously designed, statically typed systems language. Build robust, hyper-fast applications with <span className="text-white">zero GC pauses</span> and <span className="text-white">strict null safety</span>.
            </p>

            <div className="flex flex-col sm:flex-row gap-6 justify-center items-center w-full max-w-2xl mx-auto">
              <Link
                to="/docs/$"
                params={{ _splat: '' }}
                className="group relative w-full sm:w-auto px-8 py-4 rounded-2xl bg-white text-black font-bold text-lg overflow-hidden transition-transform hover:scale-105 active:scale-95"
              >
                <div className="absolute inset-0 bg-gradient-to-r from-emerald-200 to-teal-200 opacity-0 group-hover:opacity-100 transition-opacity" />
                <span className="relative flex items-center justify-center gap-2">
                  Get Started <PlayCircle className="w-5 h-5" />
                </span>
              </Link>
              <div className="relative flex items-center w-full sm:w-auto group">
                <div className="absolute -inset-1 rounded-2xl bg-gradient-to-r from-indigo-500/30 to-cyan-500/30 blur-md opacity-50 group-hover:opacity-100 transition-opacity" />
                <code className="relative w-full sm:w-auto px-6 py-4 rounded-2xl bg-black/60 border border-white/10 font-mono text-gray-300 font-medium text-lg whitespace-nowrap backdrop-blur-xl flex items-center gap-3">
                  <Terminal className="w-5 h-5 text-indigo-400" />
                  curl -fsSL https://raw.githubusercontent.com/pace-lang/pace/main/installer/install.sh | bash
                </code>
              </div>
            </div>
          </div>
        </section>

        {/* Feature Showcase Grid */}
        <section className="relative w-full max-w-7xl mx-auto px-4 py-24 z-10">
          <div className="grid md:grid-cols-2 lg:grid-cols-3 gap-6">

            {/* Card 1 */}
            <div className="group relative rounded-3xl p-[1px] overflow-hidden bg-gradient-to-b from-white/10 to-transparent hover:from-emerald-500/50 hover:to-teal-500/10 transition-colors duration-500">
              <div className="h-full w-full rounded-[23px] bg-black/80 backdrop-blur-xl p-8 flex flex-col relative overflow-hidden">
                <div className="absolute top-0 right-0 p-6 opacity-10 group-hover:opacity-20 group-hover:scale-110 transition-all text-emerald-400">
                  <ShieldCheck className="w-24 h-24" />
                </div>
                <div className="w-14 h-14 rounded-2xl bg-gradient-to-br from-emerald-500/20 to-teal-500/20 border border-emerald-500/30 flex items-center justify-center text-emerald-400 mb-6 group-hover:scale-110 transition-transform shadow-[0_0_15px_rgba(52,211,153,0.2)]">
                  <ShieldCheck className="w-7 h-7" />
                </div>
                <h3 className="text-2xl font-bold text-white mb-4">Iron-Clad Safety</h3>
                <p className="text-gray-400 leading-relaxed text-base">
                  Strict compile-time null safety (`T?`) guarantees you'll never encounter a NullPointerException. Exhaustive pattern matching prevents unhandled states.
                </p>
              </div>
            </div>

            {/* Card 2 */}
            <div className="group relative rounded-3xl p-[1px] overflow-hidden bg-gradient-to-b from-white/10 to-transparent hover:from-indigo-500/50 hover:to-cyan-500/10 transition-colors duration-500">
              <div className="h-full w-full rounded-[23px] bg-black/80 backdrop-blur-xl p-8 flex flex-col relative overflow-hidden">
                <div className="absolute top-0 right-0 p-6 opacity-10 group-hover:opacity-20 group-hover:scale-110 transition-all text-indigo-400">
                  <Zap className="w-24 h-24" />
                </div>
                <div className="w-14 h-14 rounded-2xl bg-gradient-to-br from-indigo-500/20 to-cyan-500/20 border border-indigo-500/30 flex items-center justify-center text-indigo-400 mb-6 group-hover:scale-110 transition-transform shadow-[0_0_15px_rgba(99,102,241,0.2)]">
                  <Zap className="w-7 h-7" />
                </div>
                <h3 className="text-2xl font-bold text-white mb-4">Zero GC Pauses</h3>
                <p className="text-gray-400 leading-relaxed text-base">
                  Memory is managed deterministically through Automatic Reference Counting (ARC). Achieve extreme low-latency execution for real-time systems.
                </p>
              </div>
            </div>

            {/* Card 3 */}
            <div className="group relative rounded-3xl p-[1px] overflow-hidden bg-gradient-to-b from-white/10 to-transparent hover:from-rose-500/50 hover:to-orange-500/10 transition-colors duration-500 md:col-span-2 lg:col-span-1">
              <div className="h-full w-full rounded-[23px] bg-black/80 backdrop-blur-xl p-8 flex flex-col relative overflow-hidden">
                <div className="absolute top-0 right-0 p-6 opacity-10 group-hover:opacity-20 group-hover:scale-110 transition-all text-rose-400">
                  <Layers className="w-24 h-24" />
                </div>
                <div className="w-14 h-14 rounded-2xl bg-gradient-to-br from-rose-500/20 to-orange-500/20 border border-rose-500/30 flex items-center justify-center text-rose-400 mb-6 group-hover:scale-110 transition-transform shadow-[0_0_15px_rgba(244,63,94,0.2)]">
                  <Layers className="w-7 h-7" />
                </div>
                <h3 className="text-2xl font-bold text-white mb-4">Multi-Paradigm</h3>
                <p className="text-gray-400 leading-relaxed text-base">
                  Enjoy the perfect blend of Object-Oriented design (Classes, Interfaces) and Functional primitives (Algebraic Data Types, Pattern Matching).
                </p>
              </div>
            </div>

          </div>
        </section>

        {/* Code Showcase Section */}
        <section className="w-full max-w-7xl mx-auto px-4 py-24 flex flex-col lg:flex-row gap-16 items-center z-10 relative">

          {/* Left Text */}
          <div className="flex-1 flex flex-col gap-8 w-full lg:max-w-xl text-left">
            <h2 className="text-4xl md:text-5xl lg:text-6xl font-extrabold leading-tight text-white tracking-tight">
              Syntax that feels <br />
              <span className="text-transparent bg-clip-text bg-gradient-to-r from-emerald-400 to-cyan-400">like home.</span>
            </h2>
            <p className="text-xl text-gray-400 leading-relaxed">
              Pace gets out of your way. With advanced type inference, clean C-family syntax, and strict safety guarantees at compile-time, you can focus on logic rather than fighting the compiler.
            </p>
            <div className="flex flex-col gap-4">
              <div className="flex items-center gap-4 bg-white/5 border border-white/10 p-4 rounded-2xl backdrop-blur-sm hover:bg-white/10 transition-colors">
                <Cpu className="w-8 h-8 text-emerald-400" />
                <div>
                  <h4 className="text-white font-bold">Native Compilation</h4>
                  <p className="text-sm text-gray-400">Compiles directly to machine code via Cranelift.</p>
                </div>
              </div>
            </div>
          </div>

          {/* Right IDE Window */}
          <div className="flex-1 w-full relative perspective-1000">
            {/* Glowing backdrop */}
            <div className="absolute -inset-4 rounded-[2rem] bg-gradient-to-tr from-emerald-500/20 via-indigo-500/20 to-teal-500/20 blur-2xl z-0 animate-pulse" />

            <div className="relative z-10 rounded-2xl border border-white/20 bg-[#0d1117]/90 backdrop-blur-2xl shadow-[0_20px_50px_rgba(0,0,0,0.5)] overflow-hidden transform transition-transform hover:-translate-y-2 hover:rotate-1 duration-500">
              {/* IDE Header */}
              <div className="flex items-center px-4 py-3 border-b border-white/10 bg-[#161b22]/80 backdrop-blur-md">
                <div className="flex space-x-2">
                  <div className="w-3.5 h-3.5 rounded-full bg-[#ff5f56] shadow-[0_0_5px_#ff5f56]" />
                  <div className="w-3.5 h-3.5 rounded-full bg-[#ffbd2e] shadow-[0_0_5px_#ffbd2e]" />
                  <div className="w-3.5 h-3.5 rounded-full bg-[#27c93f] shadow-[0_0_5px_#27c93f]" />
                </div>
                <div className="mx-auto text-xs text-gray-400 font-mono tracking-wider flex items-center gap-2">
                  <Terminal className="w-3 h-3" /> main.pace
                </div>
              </div>

              {/* IDE Body */}
              <div className="p-6 md:p-8 overflow-x-auto text-left relative">
                <div className="absolute top-0 left-0 w-8 h-full bg-[#161b22]/50 border-r border-white/5 flex flex-col items-center py-6 md:py-8 text-gray-600 font-mono text-sm leading-loose select-none">
                  <span>1</span><span>2</span><span>3</span><span>4</span>
                </div>
                <pre className="text-sm md:text-base font-mono leading-loose pl-10">
                  <code className="text-gray-300">
                    <span className="text-[#ff7b72] font-semibold">func</span> <span className="text-[#d2a8ff]">main</span>() {'{\n'}
                    {'    '}<span className="text-[#d2a8ff]">print</span>(<span className="text-[#a5d6ff]">"✨ Welcome to Pace — let's make something great."</span>);{'\n'}
                    {'}'}
                  </code>
                </pre>
              </div>
            </div>
          </div>
        </section>

      </main>
    </HomeLayout>
  );
}

