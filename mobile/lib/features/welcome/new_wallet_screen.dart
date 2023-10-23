import 'package:font_awesome_flutter/font_awesome_flutter.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/global_keys.dart';
import 'package:get_10101/common/loading_screen.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/features/welcome/seed_import_screen.dart';
import 'package:get_10101/ffi.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/common/scrollable_safe_area.dart';
import 'package:get_10101/util/file.dart';
import 'package:go_router/go_router.dart';

class NewWalletScreen extends StatefulWidget {
  static const route = "/new-wallet";
  static const label = "Welcome";

  const NewWalletScreen({Key? key}) : super(key: key);

  @override
  State<NewWalletScreen> createState() => _NewWalletScreenState();
}

class _NewWalletScreenState extends State<NewWalletScreen> {
  final GlobalKey<FormState> _formKey = GlobalKey<FormState>();

  bool buttonsDisabled = false;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
        body: ScrollableSafeArea(
            child: Form(
      key: _formKey,
      child: Container(
        color: Colors.white,
        padding: const EdgeInsets.all(20.0),
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          crossAxisAlignment: CrossAxisAlignment.center,
          children: <Widget>[
            const Spacer(),
            Center(
              child: Image.asset('assets/10101_logo_icon.png', width: 150, height: 150),
            ),
            const Spacer(),
            Column(children: [
              ElevatedButton(
                  // TODO: block the button until the backend is started
                  onPressed: buttonsDisabled
                      ? null
                      : () async {
                          setState(() {
                            buttonsDisabled = true;
                          });
                          final seedPath = await getSeedFilePath();
                          await api.initNewMnemonic(targetSeedFilePath: seedPath).then((value) {
                            GoRouter.of(context).go(LoadingScreen.route);
                          }).catchError((error) {
                            logger.e("Could not create seed", error: error);
                            showSnackBar(ScaffoldMessenger.of(rootNavigatorKey.currentContext!),
                                "Failed to create seed: $error");
                          });

                          // In case there was an error and we did not go forward, we want to be able to click the button again.
                          setState(() {
                            buttonsDisabled = false;
                          });
                        },
                  style: ButtonStyle(
                    padding: MaterialStateProperty.all<EdgeInsets>(const EdgeInsets.all(15)),
                    backgroundColor: MaterialStateProperty.all<Color>(tenTenOnePurple),
                    shape: MaterialStateProperty.all<RoundedRectangleBorder>(
                      RoundedRectangleBorder(
                        borderRadius: BorderRadius.circular(18.0),
                        side: const BorderSide(color: tenTenOnePurple),
                      ),
                    ),
                  ),
                  child: const Wrap(
                    children: <Widget>[
                      Icon(
                        FontAwesomeIcons.vault,
                        color: Colors.white,
                        size: 24.0,
                      ),
                      SizedBox(
                        width: 10,
                      ),
                      Text(
                        "Create wallet",
                        style: TextStyle(fontSize: 20, color: Colors.white),
                      ),
                    ],
                  )),
              const SizedBox(height: 20),
              ElevatedButton(
                onPressed: buttonsDisabled
                    ? null
                    : () {
                        setState(() {
                          buttonsDisabled = true;
                          GoRouter.of(context).go(SeedPhraseImporter.route);
                          buttonsDisabled = false;
                        });
                      },
                style: ButtonStyle(
                  padding: MaterialStateProperty.all<EdgeInsets>(const EdgeInsets.all(15)),
                  backgroundColor: MaterialStateProperty.all<Color>(Colors.white),
                  shape: MaterialStateProperty.all<RoundedRectangleBorder>(
                    RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(18.0),
                      side: const BorderSide(color: tenTenOnePurple),
                    ),
                  ),
                ),
                child: const Wrap(
                  children: <Widget>[
                    Icon(
                      FontAwesomeIcons.download,
                      color: tenTenOnePurple,
                      size: 24.0,
                    ),
                    SizedBox(
                      width: 10,
                    ),
                    Text(
                      "Restore wallet",
                      style: TextStyle(
                          fontSize: 20, color: Colors.black), // Set the text color to black
                    ),
                  ],
                ),
              )
            ]),
          ],
        ),
      ),
    )));
  }

  @override
  void initState() {
    super.initState();
  }
}
