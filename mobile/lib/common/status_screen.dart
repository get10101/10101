import 'package:flutter/material.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart';
import 'package:get_10101/common/scrollable_safe_area.dart';
import 'package:get_10101/common/service_status_notifier.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:provider/provider.dart';

class StatusScreen extends StatefulWidget {
  const StatusScreen({required this.fromRoute, super.key});

  final String fromRoute;

  @override
  State<StatusScreen> createState() => _StatusScreenState();
}

class _StatusScreenState extends State<StatusScreen> {
  @override
  Widget build(BuildContext context) {
    ServiceStatusNotifier serviceStatusNotifier = context.watch<ServiceStatusNotifier>();

    final orderbookStatus =
        statusToString(serviceStatusNotifier.getServiceStatus(Service.Orderbook));
    final coordinatorStatus =
        statusToString(serviceStatusNotifier.getServiceStatus(Service.Coordinator));
    final overallStatus = statusToString(serviceStatusNotifier.overall());

    return Scaffold(
      appBar: AppBar(title: const Text("Status")),
      body: ScrollableSafeArea(
        child: Center(
            child: Padding(
                padding: const EdgeInsets.all(32.0),
                child: Column(
                  children: [
                    const SizedBox(height: 20),
                    ValueDataRow(
                      type: ValueType.text,
                      value: overallStatus,
                      label: "App health",
                    ),
                    const Divider(),
                    ValueDataRow(
                      type: ValueType.text,
                      value: orderbookStatus,
                      label: "Orderbook",
                    ),
                    const SizedBox(height: 10),
                    ValueDataRow(
                      type: ValueType.text,
                      value: coordinatorStatus,
                      label: "LSP",
                    ),
                  ],
                ))),
      ),
    );
  }

  @override
  void initState() {
    super.initState();
  }
}

String statusToString(ServiceStatus enumValue) {
  switch (enumValue) {
    case ServiceStatus.Offline:
      return "Offline";
    case ServiceStatus.Online:
      return "Online";
    case ServiceStatus.Unknown:
      return "Unknown";
    default:
      throw Exception("Unknown enum value: $enumValue");
  }
}
