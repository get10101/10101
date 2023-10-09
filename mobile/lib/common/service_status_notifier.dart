import 'package:flutter/material.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/logger/logger.dart';

class ServiceStatusNotifier extends ChangeNotifier implements Subscriber {
  Map<bridge.Service, bridge.ServiceStatus> services = <bridge.Service, bridge.ServiceStatus>{};

  ServiceStatusNotifier();

  bridge.ServiceStatus getServiceStatus(bridge.Service service) {
    return services[service] ?? bridge.ServiceStatus.Unknown;
  }

  /// Overall health status of the application
  bridge.ServiceStatus overall() {
    return foldValues(services);
  }

  @override
  void notify(bridge.Event event) {
    if (event is bridge.Event_ServiceHealthUpdate) {
      logger.t("Received event: ${event.toString()}");
      var update = event.field0;
      services[update.service] = update.status;

      notifyListeners();
    } else {
      logger.w("Received unexpected event: ${event.toString()}");
    }
  }
}

bridge.ServiceStatus foldValues(Map<bridge.Service, bridge.ServiceStatus> map) {
  if (map.isEmpty) {
    // App is offline at startup
    return bridge.ServiceStatus.Offline;
  }
  return map.values.fold(bridge.ServiceStatus.Online, (previousValue, element) {
    if (previousValue == bridge.ServiceStatus.Offline || element == bridge.ServiceStatus.Offline) {
      return bridge.ServiceStatus.Offline;
    } else if (previousValue == bridge.ServiceStatus.Unknown ||
        element == bridge.ServiceStatus.Unknown) {
      return bridge.ServiceStatus.Unknown;
    } else {
      return bridge.ServiceStatus.Online;
    }
  });
}
