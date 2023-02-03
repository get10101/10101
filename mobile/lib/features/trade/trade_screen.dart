import 'package:flutter/material.dart';

class TradeScreen extends StatefulWidget {
  static const route = "/trade";
  static const label = "Trade";

  const TradeScreen({Key? key}) : super(key: key);

  @override
  State<TradeScreen> createState() => _TradeScreenState();
}

class _TradeScreenState extends State<TradeScreen> {
  @override
  Widget build(BuildContext context) {
    var fabShape = RoundedRectangleBorder(borderRadius: BorderRadius.circular(10.0));
    double fabWidth = 100.0;

    return Scaffold(
        body: ListView(
          padding: const EdgeInsets.only(left: 25, right: 25),
          children: const [Center(child: Text("Trade Screen"))],
        ),
        floatingActionButtonLocation: FloatingActionButtonLocation.centerDocked,
        floatingActionButton: Padding(
          padding: const EdgeInsets.all(8.0),
          child: Row(
            mainAxisAlignment: MainAxisAlignment.center,
            children: <Widget>[
              SizedBox(
                  width: fabWidth,
                  child: FloatingActionButton.extended(
                    heroTag: "btnBuy",
                    onPressed: () {
                      showModalBottomSheet<void>(
                        useRootNavigator: true,
                        backgroundColor: Colors.green.shade50,
                        context: context,
                        builder: (BuildContext context) {
                          return SizedBox(
                            height: 200,
                            child: Center(
                              child: Column(
                                mainAxisAlignment: MainAxisAlignment.center,
                                mainAxisSize: MainAxisSize.min,
                                children: const <Widget>[
                                  Text('Buy Sheet'),
                                ],
                              ),
                            ),
                          );
                        },
                      );
                    },
                    label: const Text("Buy"),
                    shape: fabShape,
                    backgroundColor: Colors.green.shade600,
                  )),
              const SizedBox(width: 20),
              SizedBox(
                  width: fabWidth,
                  child: FloatingActionButton.extended(
                    heroTag: "btnSell",
                    onPressed: () {
                      showModalBottomSheet<void>(
                        useRootNavigator: true,
                        backgroundColor: Colors.red.shade50,
                        context: context,
                        builder: (BuildContext context) {
                          return SizedBox(
                            height: 200,
                            child: Center(
                              child: Column(
                                mainAxisAlignment: MainAxisAlignment.center,
                                mainAxisSize: MainAxisSize.min,
                                children: const <Widget>[
                                  Text('Sell Sheet'),
                                ],
                              ),
                            ),
                          );
                        },
                      );
                    },
                    label: const Text("Sell"),
                    shape: fabShape,
                    backgroundColor: Colors.red.shade600,
                  )),
            ],
          ),
        ));
  }
}
