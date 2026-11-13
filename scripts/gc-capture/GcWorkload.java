import java.util.ArrayList;
import java.util.List;

/**
 * Rate-limited allocation workload used to capture real GC logs (feat-068).
 *
 * Usage: java [gc flags] GcWorkload <seconds> [modes]
 *   modes: comma list of churn, humongous, systemgc, leak (default churn)
 * Tuning system properties:
 *   jblock.allocMBps        short-lived allocation rate (default 40)
 *   jblock.leakMBps         retained allocation rate in leak mode (default 1)
 *   jblock.humongousEveryMs 4 MiB array interval in humongous mode (default 200)
 *   jblock.systemGcEveryMs  System.gc() interval in systemgc mode (default 2000)
 */
public class GcWorkload {
    static volatile Object sink;

    public static void main(String[] args) throws Exception {
        long seconds = args.length > 0 ? Long.parseLong(args[0]) : 5;
        String modes = args.length > 1 ? args[1] : "churn";
        boolean humongous = modes.contains("humongous");
        boolean systemGc = modes.contains("systemgc");
        boolean leak = modes.contains("leak");
        double allocBps = Double.parseDouble(System.getProperty("jblock.allocMBps", "40")) * (1 << 20);
        double leakBps = Double.parseDouble(System.getProperty("jblock.leakMBps", "1")) * (1 << 20);
        long humongousEveryNs = Long.getLong("jblock.humongousEveryMs", 200) * 1_000_000L;
        long systemGcEveryNs = Long.getLong("jblock.systemGcEveryMs", 2000) * 1_000_000L;

        List<byte[]> retained = new ArrayList<>();
        long start = System.nanoTime();
        long end = start + seconds * 1_000_000_000L;
        long allocated = 0;
        long leaked = 0;
        long lastHumongous = start;
        long lastSystemGc = start;
        try {
            while (true) {
                long now = System.nanoTime();
                if (now >= end) break;
                double elapsed = (now - start) / 1e9;
                // Short-lived churn, paced to the target rate.
                while (allocated < elapsed * allocBps) {
                    sink = new byte[1024];
                    allocated += 1024 + 16;
                }
                if (leak) {
                    while (leaked < elapsed * leakBps) {
                        retained.add(new byte[16 << 10]);
                        leaked += 16 << 10;
                    }
                }
                if (humongous && now - lastHumongous >= humongousEveryNs) {
                    sink = new byte[4 << 20];
                    lastHumongous = now;
                }
                if (systemGc && now - lastSystemGc >= systemGcEveryNs) {
                    System.gc();
                    lastSystemGc = now;
                }
                Thread.sleep(2);
            }
        } catch (OutOfMemoryError oom) {
            int n = retained.size();
            retained.clear();
            System.err.println("OOM after retaining " + n + " chunks");
        }
        System.out.println("done allocatedMiB=" + (allocated >> 20) + " retained=" + retained.size());
    }
}
