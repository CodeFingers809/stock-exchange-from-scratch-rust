"use client";

import React from "react";

interface VolumeProfileProps {
  data: { price: number; volume: number; buyVolume: number; sellVolume: number }[];
}

export function VolumeProfile({ data }: VolumeProfileProps) {
  const maxVol = Math.max(...data.map((d) => d.volume), 1);

  return (
    <div className="flex flex-col h-full bg-[#0a0e17] border-l border-[#1e2740]">
      <div className="px-3 py-2 border-b border-[#1e2740]">
        <span className="text-[10px] font-mono tracking-widest text-[#4a5568] uppercase">
          Volume Profile
        </span>
      </div>
      <div className="flex-1 flex flex-col justify-around px-2 py-1 gap-0.5">
        {data.map((row, idx) => {
          const buyW = (row.buyVolume / maxVol) * 100;
          const sellW = (row.sellVolume / maxVol) * 100;
          const isPOC = row.volume === Math.max(...data.map((d) => d.volume));

          return (
            <div key={idx} className="flex items-center gap-1.5 group">
              <span className={`w-[52px] text-right font-mono text-[10px] tabular-nums ${
                isPOC ? "text-[#f59e0b]" : "text-[#4a5568] group-hover:text-[#8494a7]"
              }`}>
                {row.price.toFixed(0)}
              </span>
              <div className="flex-1 h-[10px] flex items-center bg-[#0f1420] relative">
                <div
                  className="h-full bg-[#00c07650]"
                  style={{ width: `${buyW}%` }}
                />
                <div
                  className="h-full bg-[#ff475750]"
                  style={{ width: `${sellW}%` }}
                />
                {isPOC && (
                  <div className="absolute inset-y-0 left-0 w-full border border-[#f59e0b30]" />
                )}
              </div>
              <span className="w-[36px] text-right font-mono text-[9px] tabular-nums text-[#4a5568]">
                {(row.volume / 1000).toFixed(1)}k
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
