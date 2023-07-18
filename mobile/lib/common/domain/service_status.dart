import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;

bridge.ServiceUpdate serviceUpdateApiDummy() {
  return const bridge.ServiceUpdate(
      service: bridge.Service.Orderbook, status: bridge.ServiceStatus.Unknown);
}
