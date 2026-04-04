/** Effects types — matching osg-core graph::types */

export interface CompressorConfig {
  enabled: boolean;
  threshold: number;
  ratio: number;
  attack: number; // ms
  release: number; // ms
  makeup: number; // dB
}

export interface GateConfig {
  enabled: boolean;
  threshold: number;
  hold: number; // ms
  attack: number; // ms
  release: number; // ms
}

export interface DeEsserConfig {
  enabled: boolean;
  frequency: number;
  threshold: number;
  reduction: number;
}

export interface LimiterConfig {
  enabled: boolean;
  ceiling: number;
  release: number; // ms
}

export interface SmartVolumeConfig {
  enabled: boolean;
  /** Target RMS level in dB (e.g., -18.0). */
  targetDb: number;
  /** Response speed: 0.0 = slow, 1.0 = fast. */
  speed: number;
  /** Maximum gain increase in dB. */
  maxGainDb: number;
}

export interface SpatialAudioConfig {
  enabled: boolean;
  /** Crossfeed amount (0.0 = none, 1.0 = full mono). */
  crossfeed: number;
  /** Stereo width (0.0 = mono, 1.0 = normal, 2.0 = extra wide). */
  width: number;
}

export interface EffectsConfig {
  compressor: CompressorConfig;
  gate: GateConfig;
  deEsser: DeEsserConfig;
  limiter: LimiterConfig;
  /** Volume boost in dB (0-12). Applied as linear gain after limiter. */
  boost: number;
  /** Smart volume (loudness normalization). */
  smartVolume: SmartVolumeConfig;
  /** Spatial audio: crossfeed + stereo width (mix-only). */
  spatial?: SpatialAudioConfig;
}
