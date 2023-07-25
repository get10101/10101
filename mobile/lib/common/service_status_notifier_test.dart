import 'package:get_10101/common/service_status_notifier.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart';
import 'package:test/test.dart';

void main() {
  test('Fold values in service map to determine overall status', () {
    Map<Service, ServiceStatus> testMap1 = <Service, ServiceStatus>{
      Service.Orderbook: ServiceStatus.Online,
      Service.Coordinator: ServiceStatus.Online,
    };
    expect(foldValues(testMap1), ServiceStatus.Online, reason: 'Everything is online');

    Map<Service, ServiceStatus> testMap2 = <Service, ServiceStatus>{
      Service.Orderbook: ServiceStatus.Online,
      Service.Coordinator: ServiceStatus.Unknown,
    };
    expect(foldValues(testMap2), ServiceStatus.Unknown, reason: 'At least one service is unknown');

    Map<Service, ServiceStatus> testMap3 = <Service, ServiceStatus>{
      Service.Orderbook: ServiceStatus.Offline,
      Service.Coordinator: ServiceStatus.Online,
    };
    expect(foldValues(testMap3), ServiceStatus.Offline, reason: 'At least one service is offline');

    Map<Service, ServiceStatus> testMap4 = <Service, ServiceStatus>{
      Service.Orderbook: ServiceStatus.Unknown,
      Service.Coordinator: ServiceStatus.Offline,
    };
    expect(foldValues(testMap4), ServiceStatus.Offline, reason: 'At least one service is offline');
  });
}
