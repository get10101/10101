import 'package:flutter/material.dart';
import 'package:get_10101/features/wallet/scanner_screen.dart';
import 'package:go_router/go_router.dart';

void showEnterDestinationModal(BuildContext context, Function onSetDestination) {
  showModalBottomSheet<void>(
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(
          top: Radius.circular(20),
        ),
      ),
      clipBehavior: Clip.antiAlias,
      isScrollControlled: true,
      useRootNavigator: true,
      context: context,
      builder: (BuildContext context) {
        return SafeArea(
            child: Padding(
          padding: EdgeInsets.only(bottom: MediaQuery.of(context).viewInsets.bottom),
          // the GestureDetector ensures that we can close the keyboard by tapping into the modal
          child: GestureDetector(
            onTap: () {
              FocusScopeNode currentFocus = FocusScope.of(context);

              if (!currentFocus.hasPrimaryFocus) {
                currentFocus.unfocus();
              }
            },
            child: SingleChildScrollView(
              child: SizedBox(
                // TODO: Find a way to make height dynamic depending on the children size
                // This is needed because otherwise the keyboard does not push the sheet up correctly
                height: 200,
                child: EnterDestinationModal(onSetDestination: onSetDestination),
              ),
            ),
          ),
        ));
      });
}

class EnterDestinationModal extends StatefulWidget {
  final Function onSetDestination;

  const EnterDestinationModal({super.key, required this.onSetDestination});

  @override
  State<EnterDestinationModal> createState() => _EnterDestinationModalState();
}

class _EnterDestinationModalState extends State<EnterDestinationModal> {
  String? destination;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(left: 20.0, top: 30.0, right: 20.0),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          TextFormField(
            decoration: InputDecoration(
                labelText: "Destination",
                hintText: "e.g. an invoice, BIP21 URI or on-chain address",
                suffixIcon: IconButton(
                    icon: const Icon(Icons.qr_code),
                    onPressed: () {
                      GoRouter.of(context).go(ScannerScreen.route);
                    })),
            onChanged: (value) {
              destination = value;
            },
          ),
          const SizedBox(height: 20),
          ElevatedButton(
              onPressed: () {
                widget.onSetDestination(destination);
                GoRouter.of(context).pop();
              },
              child: const Text("Set Destination", style: TextStyle(fontSize: 16)))
        ],
      ),
    );
  }
}
