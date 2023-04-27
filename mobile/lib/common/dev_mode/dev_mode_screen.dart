import 'package:f_logs/model/flog/flog.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/ffi.dart' as rust;
import 'package:go_router/go_router.dart';

class DevModeScreen extends StatefulWidget {
  const DevModeScreen({required this.fromRoute, super.key});

  final String fromRoute;

  @override
  State<DevModeScreen> createState() => _DevModeScreenState();
}

class _DevModeScreenState extends State<DevModeScreen> {
  @override
  void initState() {
    super.initState();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text("Developers")),
      body: ListView(
          children: ListTile.divideTiles(context: context, tiles: [
        InkWell(
          child: ListTile(
            title: Row(children: [
              RichText(
                text: const TextSpan(
                    text: 'Close position',
                    style: TextStyle(
                        fontWeight: FontWeight.w600,
                        fontSize: 18,
                        color: Colors.black,
                        fontFamily: "Arial")),
                textAlign: TextAlign.right,
              )
            ]),
            trailing: const Icon(Icons.arrow_forward_ios),
            onTap: () {
              GoRouter.of(context).go('${widget.fromRoute}/${DevClosePositionScreen.subRouteName}');
            },
          ),
        ),
      ]).toList()),
    );
  }
}

class DevClosePositionScreen extends StatefulWidget {
  static const subRouteName = "close_position";

  const DevClosePositionScreen({super.key});

  @override
  State<DevClosePositionScreen> createState() => _DevClosePositionScreenState();
}

class _DevClosePositionScreenState extends State<DevClosePositionScreen> {
  bool isLoading = false;

  @override
  void initState() {
    super.initState();
  }

  Future<void> closePosition() async {
    setState(() {
      isLoading = true;
    });

    try {
      await Future.delayed(const Duration(seconds: 1));
      await rust.api.closePosition();
    } catch (error) {
      FLog.error(text: "Failed to close position: $error");
      ScaffoldMessenger.of(context)
          .showSnackBar(const SnackBar(content: Text('Could not close position')));
      return;
    } finally {
      setState(() {
        isLoading = false;
      });
    }

    if (context.mounted) {
      ScaffoldMessenger.of(context).showSnackBar(const SnackBar(content: Text('Position closed')));
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
        appBar: AppBar(title: const Text("Close position")),
        body: SafeArea(
          child: Padding(
            padding: const EdgeInsets.all(20.0),
            child: Column(children: [
              Center(
                child: RichText(
                    text: const TextSpan(
                        style: TextStyle(color: Colors.black, fontSize: 18),
                        children: [
                      TextSpan(
                          text:
                              "If you proceed, the app will assume that there is no underlying open DLC and delete the currently open position from the database.\n\n"),
                      TextSpan(
                          text: "This operation is irreversible.",
                          style: TextStyle(fontWeight: FontWeight.bold)),
                    ])),
              ),
              Column(children: [
                Padding(
                  padding: const EdgeInsets.all(32.0),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    mainAxisAlignment: MainAxisAlignment.end,
                    children: [
                      ElevatedButton.icon(
                          onPressed: isLoading ? null : closePosition,
                          label: isLoading ? const Text('') : const Text("Proceed"),
                          icon: isLoading
                              ? const SizedBox(
                                  width: 20, height: 20, child: CircularProgressIndicator())
                              : const SizedBox.shrink()),
                    ],
                  ),
                ),
              ])
            ]),
          ),
        ));
  }
}
