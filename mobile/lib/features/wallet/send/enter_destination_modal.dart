import 'package:flutter/material.dart';
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
      useRootNavigator: false,
      context: context,
      builder: (BuildContext context) {
        return SingleChildScrollView(
            child: SizedBox(
                height: 230,
                child: Scaffold(body: EnterDestinationModal(onSetDestination: onSetDestination))));
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
    return SafeArea(
      child: Padding(
        padding: const EdgeInsets.only(left: 20.0, top: 35.0, right: 20.0),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            TextFormField(
              decoration: const InputDecoration(
                labelText: "Destination",
                hintText: "e.g an invoice, bip21 uri or on-chain address",
              ),
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
      ),
    );
  }
}
