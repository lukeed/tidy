// Reads the timings that hyperfine 2 writes with --export-json, in seconds.

export type Timing = {
  name: string;
  mean: number;
  median: number;
  min: number;
  user: number;
  system: number;
};

type Stat = { mean: number; median: number; min: number };
type Measured = {
  name: string;
  summary: { time_wall_clock: Stat; time_user: Stat; time_system: Stat };
};

export async function timings(file: string): Promise<Timing[]> {
  let { results } = (await Bun.file(file).json()) as { results: Measured[] };
  return results.map(({ name, summary }) => ({
    name,
    mean: summary.time_wall_clock.mean,
    median: summary.time_wall_clock.median,
    min: summary.time_wall_clock.min,
    user: summary.time_user.mean,
    system: summary.time_system.mean,
  }));
}
