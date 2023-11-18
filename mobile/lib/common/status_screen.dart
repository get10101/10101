import 'package:flutter/material.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart';
import 'package:get_10101/common/service_status_notifier.dart';
import 'package:get_10101/common/settings/settings_screen.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

class StatusScreen extends StatefulWidget {
  static const route = "${SettingsScreen.route}/$subRouteName";
  static const subRouteName = "status";

  const StatusScreen({super.key});

  @override
  State<StatusScreen> createState() => _StatusScreenState();
}

class _StatusScreenState extends State<StatusScreen> {
  @override
  Widget build(BuildContext context) {
    ServiceStatusNotifier serviceStatusNotifier = context.watch<ServiceStatusNotifier>();

    final orderbookStatus =
        serviceStatusToString(serviceStatusNotifier.getServiceStatus(Service.Orderbook));
    final coordinatorStatus =
        serviceStatusToString(serviceStatusNotifier.getServiceStatus(Service.Coordinator));
    final overallStatus = serviceStatusToString(serviceStatusNotifier.overall());

    return Scaffold(
        body: Container(
            padding: const EdgeInsets.only(top: 20, left: 10, right: 10),
            child: SafeArea(
                child: Column(children: [
              Row(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  Expanded(
                    child: Stack(
                      children: [
                        GestureDetector(
                            child: Container(
                                alignment: AlignmentDirectional.topStart,
                                decoration: BoxDecoration(
                                    color: Colors.transparent,
                                    borderRadius: BorderRadius.circular(10)),
                                width: 70,
                                child: const Icon(
                                  Icons.arrow_back_ios_new_rounded,
                                  size: 22,
                                )),
                            onTap: () => GoRouter.of(context).pop()),
                        const Row(
                          mainAxisAlignment: MainAxisAlignment.center,
                          children: [
                            Text(
                              "Status",
                              style: TextStyle(fontWeight: FontWeight.w500, fontSize: 20),
                            ),
                          ],
                        ),
                      ],
                    ),
                  ),
                ],
              ),
              const SizedBox(
                height: 20,
              ),
              Padding(
                padding: const EdgeInsets.all(10.0),
                child: Column(
                  children: [
                    ValueDataRow(
                      type: ValueType.text,
                      value: overallStatus,
                      label: "Services",
                      labelTextStyle: const TextStyle(fontSize: 18),
                      valueTextStyle: const TextStyle(fontWeight: FontWeight.bold, fontSize: 18),
                    ),
                    const Divider(),
                    ValueDataRow(
                      type: ValueType.text,
                      value: orderbookStatus,
                      label: "Orderbook",
                      labelTextStyle: const TextStyle(fontSize: 18),
                      valueTextStyle: const TextStyle(fontWeight: FontWeight.bold, fontSize: 18),
                    ),
                    const SizedBox(height: 10),
                    ValueDataRow(
                      type: ValueType.text,
                      value: coordinatorStatus,
                      label: "LSP",
                      labelTextStyle: const TextStyle(fontSize: 18),
                      valueTextStyle: const TextStyle(fontWeight: FontWeight.bold, fontSize: 18),
                    ),
                  ],
                ),
              )
            ]))));
  }

  @override
  void initState() {
    super.initState();
  }
}

String serviceStatusToString(ServiceStatus enumValue) {
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
