import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:font_awesome_flutter/font_awesome_flutter.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/features/wallet/domain/destination.dart';
import 'package:get_10101/features/wallet/domain/wallet_type.dart';
import 'package:get_10101/features/wallet/send/send_onchain_screen.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';
import 'package:qr_code_scanner/qr_code_scanner.dart';

class ScannerScreen extends StatefulWidget {
  static const route = "${WalletScreen.route}/$subRouteName";
  static const subRouteName = "scanner";

  const ScannerScreen({Key? key}) : super(key: key);

  @override
  State<StatefulWidget> createState() => _ScannerScreenState();
}

class _ScannerScreenState extends State<ScannerScreen> {
  QRViewController? controller;
  final GlobalKey qrKey = GlobalKey(debugLabel: 'QR');

  final _formKey = GlobalKey<FormState>();
  String? _encodedDestination;
  Destination? _destination;

  final textEditingController = TextEditingController();

  bool cameraPermission = !kDebugMode;

  @override
  void reassemble() {
    super.reassemble();
    if (kDebugMode) {
      // In order to get hot reload to work we need to pause the camera if the platform
      // is android, or resume the camera if the platform is iOS.
      if (Platform.isAndroid) {
        controller!.pauseCamera();
      }
      controller!.resumeCamera();
    }
  }

  @override
  Widget build(BuildContext context) {
    final walletService = context.read<WalletChangeNotifier>().service;

    return Scaffold(
        body: SafeArea(
      child: Container(
        margin: const EdgeInsets.only(top: 10),
        child: Column(children: [
          Container(
            margin: const EdgeInsets.only(left: 10),
            child: Row(
              mainAxisAlignment: MainAxisAlignment.start,
              children: [
                GestureDetector(
                  child: Container(
                      alignment: AlignmentDirectional.topStart,
                      decoration: BoxDecoration(
                          color: Colors.transparent, borderRadius: BorderRadius.circular(10)),
                      width: 70,
                      child: const Icon(
                        Icons.arrow_back_ios_new_rounded,
                        size: 22,
                      )),
                  onTap: () {
                    GoRouter.of(context).pop();
                  },
                ),
              ],
            ),
          ),
          const SizedBox(height: 20),
          Expanded(
              child: !cameraPermission
                  ? const Padding(
                      padding: EdgeInsets.only(left: 25, right: 25),
                      child: Column(
                          mainAxisAlignment: MainAxisAlignment.center,
                          crossAxisAlignment: CrossAxisAlignment.center,
                          children: [
                            Icon(FontAwesomeIcons.eyeSlash, size: 55),
                            SizedBox(height: 20),
                            Text(
                                "To scan a QR code you must give 10101 permission to access the camera.",
                                textAlign: TextAlign.center)
                          ]),
                    )
                  : QRView(
                      key: qrKey,
                      onQRViewCreated: (controller) {
                        setState(() {
                          this.controller = controller;
                        });
                        controller.scannedDataStream.listen((scanData) {
                          if (scanData.code != null) {
                            setState(() {
                              _encodedDestination = scanData.code;
                              textEditingController.text = _encodedDestination ?? "";
                            });
                            walletService
                                .decodeDestination(scanData.code ?? "")
                                .then((destination) {
                              _destination = destination;
                              if (_formKey.currentState!.validate()) {
                                switch (destination!.getWalletType()) {
                                  case WalletType.offChain:
                                    _showNoLightningDialog(context);
                                  case WalletType.onChain:
                                    GoRouter.of(context)
                                        .go(SendOnChainScreen.route, extra: destination);
                                }
                              }
                            });
                          }
                        });
                      },
                      overlay: QrScannerOverlayShape(
                          borderColor: Colors.red,
                          borderRadius: 10,
                          borderLength: 30,
                          borderWidth: 10,
                          cutOutSize: 300.0),
                      formatsAllowed: const [BarcodeFormat.qrcode],
                      onPermissionSet: (ctrl, p) => _onPermissionSet(context, ctrl, p),
                    )),
          Container(
            margin: const EdgeInsets.all(20),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                const Text("Destination"),
                const SizedBox(height: 5),
                Form(
                  key: _formKey,
                  child: TextFormField(
                    validator: (value) {
                      if (_destination == null) {
                        return "Invalid destination";
                      }

                      return null;
                    },
                    onChanged: (value) => _encodedDestination = value,
                    controller: textEditingController,
                    decoration: InputDecoration(
                      filled: true,
                      fillColor: Colors.grey.shade200,
                      suffixIcon: GestureDetector(
                        onTap: () async {
                          final data = await Clipboard.getData("text/plain");

                          if (data?.text != null) {
                            setState(() {
                              _encodedDestination = data!.text!;
                              textEditingController.text = _encodedDestination!;
                            });
                          }
                        },
                        child: const Icon(Icons.paste_rounded, color: tenTenOnePurple, size: 18),
                      ),
                      enabledBorder:
                          OutlineInputBorder(borderSide: BorderSide(color: Colors.grey.shade200)),
                      border:
                          OutlineInputBorder(borderSide: BorderSide(color: Colors.grey.shade200)),
                    ),
                  ),
                ),
                const SizedBox(height: 20),
                SizedBox(
                  width: MediaQuery.of(context).size.width * 0.9,
                  child: ElevatedButton(
                      onPressed: () {
                        walletService
                            .decodeDestination(_encodedDestination ?? "")
                            .then((destination) {
                          _destination = destination;

                          if (_formKey.currentState!.validate()) {
                            switch (destination!.getWalletType()) {
                              case WalletType.offChain:
                                _showNoLightningDialog(context);
                              case WalletType.onChain:
                                GoRouter.of(context)
                                    .go(SendOnChainScreen.route, extra: destination);
                            }
                          }
                        });
                      },
                      style: ButtonStyle(
                          padding: MaterialStateProperty.all<EdgeInsets>(const EdgeInsets.all(15)),
                          backgroundColor: MaterialStateProperty.resolveWith((states) {
                            if (states.contains(MaterialState.disabled)) {
                              return tenTenOnePurple.shade100;
                            } else {
                              return tenTenOnePurple;
                            }
                          }),
                          shape: MaterialStateProperty.resolveWith((states) {
                            if (states.contains(MaterialState.disabled)) {
                              return RoundedRectangleBorder(
                                borderRadius: BorderRadius.circular(30.0),
                                side: BorderSide(color: tenTenOnePurple.shade100),
                              );
                            } else {
                              return RoundedRectangleBorder(
                                borderRadius: BorderRadius.circular(30.0),
                                side: const BorderSide(color: tenTenOnePurple),
                              );
                            }
                          })),
                      child: const Text(
                        "Next",
                        style: TextStyle(fontSize: 18, color: Colors.white),
                      )),
                ),
              ],
            ),
          ),
        ]),
      ),
    ));
  }

  void _onPermissionSet(BuildContext context, QRViewController ctrl, bool permission) {
    logger.d('${DateTime.now().toIso8601String()}_onPermissionSet $permission');
    setState(() => cameraPermission = permission);
  }

  @override
  void dispose() {
    controller?.dispose();
    super.dispose();
  }
}

Future<void> _showNoLightningDialog(BuildContext context) async {
  return showDialog<void>(
    context: context,
    barrierDismissible: false,
    builder: (BuildContext context) {
      return AlertDialog(
        title: const Text('Can\'t pay invoice'),
        content: const SingleChildScrollView(
          child: ListBody(
            children: <Widget>[
              Text('For the time being we have disabled paying Lightning invoices.'),
              Text('Thank you for your understanding.'),
            ],
          ),
        ),
        actions: <Widget>[
          TextButton(
            child: const Text('Ok'),
            onPressed: () {
              Navigator.of(context).pop();
            },
          ),
        ],
      );
    },
  );
}
