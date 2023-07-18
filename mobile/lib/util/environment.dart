import 'package:get_10101/bridge_generated/bridge_definitions.dart';

class Environment {
  static Config parse() {
    String host = const String.fromEnvironment('COORDINATOR_HOST', defaultValue: '127.0.0.1');
    // coordinator PK is derived from our checked in regtest maker seed
    String coordinatorPublicKey = const String.fromEnvironment("COORDINATOR_PK",
        defaultValue: "02dd6abec97f9a748bf76ad502b004ce05d1b2d1f43a9e76bd7d85e767ffb022c9");
    int lightningPort = const int.fromEnvironment("COORDINATOR_PORT_LIGHTNING", defaultValue: 9045);
    int httpPort = const int.fromEnvironment("COORDINATOR_PORT_HTTP", defaultValue: 8000);
    String esploraEndpoint =
        const String.fromEnvironment("ESPLORA_ENDPOINT", defaultValue: "http://127.0.0.1:3000");
    String network = const String.fromEnvironment('NETWORK', defaultValue: "regtest");
    String oracleEndpoint =
        const String.fromEnvironment("ORACLE_ENDPOINT", defaultValue: "http://127.0.0.1:8081");
    String oraclePubkey = const String.fromEnvironment("ORACLE_PUBKEY",
        defaultValue: "16f88cf7d21e6c0f46bcbc983a4e3b19726c6c98858cc31c83551a88fde171c0");

    String p2pEndpoint = const String.fromEnvironment('COORDINATOR_P2P_ENDPOINT');
    if (p2pEndpoint.contains("@")) {
      final split = p2pEndpoint.split("@");
      coordinatorPublicKey = split[0];
      if (split[1].contains(':')) {
        host = split[1].split(':')[0];
        lightningPort = int.parse(split[1].split(':')[1]);
      }
    }

    int healthCheckIntervalSeconds =
        const int.fromEnvironment('HEALTH_CHECK_INTERVAL_SECONDS', defaultValue: 10);

    return Config(
      host: host,
      esploraEndpoint: esploraEndpoint,
      coordinatorPubkey: coordinatorPublicKey,
      p2PPort: lightningPort,
      httpPort: httpPort,
      network: network,
      oracleEndpoint: oracleEndpoint,
      oraclePubkey: oraclePubkey,
      healthCheckIntervalSecs: healthCheckIntervalSeconds,
    );
  }
}
