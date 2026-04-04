/** EQ types — matching osg-core graph::types */

export type FilterType = "peaking" | "lowShelf" | "highShelf" | "lowPass" | "highPass" | "notch";

export interface EqBand {
  enabled: boolean;
  filterType: FilterType;
  frequency: number;
  gain: number;
  q: number;
}

export interface EqConfig {
  enabled: boolean;
  bands: EqBand[];
}
