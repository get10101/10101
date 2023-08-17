import 'package:flutter/material.dart';
import 'package:get_10101/util/preferences.dart';

/// Widget to toggle between mainnet and regtest (requires app restart).
class NetworkToggleButton extends StatefulWidget {
  const NetworkToggleButton({super.key});

  @override
  NetworkToggleButtonState createState() => NetworkToggleButtonState();
}

class NetworkToggleButtonState extends State<NetworkToggleButton> {
  @override
  void initState() {
    Preferences.instance.getNetwork().then((network) {
      setState(() {
        _currentNetwork = network;
      });
    });
    super.initState();
  }

  Network _currentNetwork = Network.mainnet;

  void _toggleNetwork() {
    setState(() {
      if (_currentNetwork == Network.mainnet) {
        _currentNetwork = Network.regtest;
      } else {
        _currentNetwork = Network.mainnet;
      }
    });

    Preferences.instance.setNetwork(_currentNetwork);

    // Show a dialog after updating the state
    showDialog(
      context: context,
      builder: (BuildContext context) {
        return AlertDialog(
          title: const Text('Network Changed'),
          content: const Text('The new setting will be applied after app restart.'),
          actions: <Widget>[
            TextButton(
              child: const Text('OK'),
              onPressed: () {
                Navigator.of(context).pop();
              },
            ),
          ],
        );
      },
    );
  }

  @override
  Widget build(BuildContext context) {
    return ElevatedButton(
      onPressed: _toggleNetwork,
      child: Text(
        'Change to ${_currentNetwork == Network.mainnet ? 'regtest' : 'mainnet'}',
      ),
    );
  }
}
